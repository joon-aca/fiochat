use std::path::{Path, PathBuf};

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{ensure_parent_exists, Config};
use crate::mcp::auth::types::StoredOAuthToken;
use crate::mcp::config::TokenStoreConfig;
use crate::utils::{base64_decode, base64_encode, resolve_home_dir};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedTokenPayload {
    version: u8,
    nonce_b64: String,
    ciphertext_b64: String,
}

pub fn load_token(
    server_name: &str,
    token_store: &TokenStoreConfig,
) -> Result<Option<StoredOAuthToken>> {
    let path = token_file_path(server_name, token_store)?;
    if !path.exists() {
        return Ok(None);
    }
    let payload_data = std::fs::read_to_string(&path)
        .with_context(|| format!("MCP oauth token store: failed to read '{}'", path.display()))?;
    let payload: EncryptedTokenPayload =
        serde_json::from_str(&payload_data).with_context(|| {
            format!(
                "MCP oauth token store: invalid payload at '{}'",
                path.display()
            )
        })?;
    if payload.version != 1 {
        bail!(
            "MCP oauth token store: unsupported payload version '{}' at '{}'",
            payload.version,
            path.display()
        );
    }
    let key = load_encryption_key(token_store)?;
    let nonce = decode_nonce(&payload.nonce_b64)?;
    let ciphertext = base64_decode(&payload.ciphertext_b64)
        .map_err(|_| anyhow!("MCP oauth token store: invalid ciphertext encoding"))?;
    let plaintext = decrypt(&key, &nonce, &ciphertext)
        .context("MCP oauth token store: failed to decrypt token payload")?;
    let token: StoredOAuthToken = serde_json::from_slice(&plaintext)
        .context("MCP oauth token store: invalid token payload JSON")?;
    Ok(Some(token))
}

pub fn save_token(
    server_name: &str,
    token_store: &TokenStoreConfig,
    token: &StoredOAuthToken,
) -> Result<()> {
    let path = token_file_path(server_name, token_store)?;
    ensure_parent_exists(&path)?;
    ensure_secure_parent_dir(path.parent())?;

    let key = load_encryption_key(token_store)?;
    let nonce = new_nonce();
    let plaintext =
        serde_json::to_vec(token).context("MCP oauth token store: failed to serialize token")?;
    let ciphertext = encrypt(&key, &nonce, &plaintext)
        .context("MCP oauth token store: failed to encrypt token payload")?;
    let payload = EncryptedTokenPayload {
        version: 1,
        nonce_b64: base64_encode(nonce),
        ciphertext_b64: base64_encode(ciphertext),
    };
    let payload_str = serde_json::to_string_pretty(&payload)
        .context("MCP oauth token store: failed to serialize encrypted payload")?;
    std::fs::write(&path, payload_str).with_context(|| {
        format!(
            "MCP oauth token store: failed to write '{}'",
            path.display()
        )
    })?;
    set_secure_file_permissions(&path)?;
    Ok(())
}

pub fn delete_token(server_name: &str, token_store: &TokenStoreConfig) -> Result<bool> {
    let path = token_file_path(server_name, token_store)?;
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path).with_context(|| {
        format!(
            "MCP oauth token store: failed to delete '{}'",
            path.display()
        )
    })?;
    Ok(true)
}

fn load_encryption_key(token_store: &TokenStoreConfig) -> Result<[u8; 32]> {
    match token_store {
        TokenStoreConfig::EncryptedFile { key_env, .. } => {
            let value = std::env::var(key_env).map_err(|_| {
                anyhow!(
                    "MCP oauth token store: env var '{}' is not set; set it to a base64-encoded 32-byte key",
                    key_env
                )
            })?;
            let decoded = base64_decode(value.trim()).map_err(|_| {
                anyhow!(
                    "MCP oauth token store: env var '{}' is not valid base64",
                    key_env
                )
            })?;
            if decoded.len() != 32 {
                bail!(
                    "MCP oauth token store: env var '{}' must decode to exactly 32 bytes",
                    key_env
                );
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            Ok(key)
        }
    }
}

fn token_file_path(server_name: &str, token_store: &TokenStoreConfig) -> Result<PathBuf> {
    let dir = match token_store {
        TokenStoreConfig::EncryptedFile { path, .. } => {
            if let Some(path) = path {
                PathBuf::from(resolve_home_dir(path))
            } else {
                Config::config_dir().join("secrets").join("mcp-oauth")
            }
        }
    };
    let server_name = sanitize_server_name(server_name);
    Ok(dir.join(format!("{server_name}.json.enc")))
}

fn sanitize_server_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn new_nonce() -> [u8; 12] {
    let uuid = Uuid::new_v4();
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&uuid.as_bytes()[0..12]);
    nonce
}

fn decode_nonce(value: &str) -> Result<[u8; 12]> {
    let bytes = base64_decode(value).map_err(|_| anyhow!("invalid nonce encoding"))?;
    if bytes.len() != 12 {
        bail!("invalid nonce length");
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes);
    Ok(nonce)
}

fn encrypt(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).context("invalid encryption key")?;
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|_| anyhow!("encryption failure"))
}

fn decrypt(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).context("invalid encryption key")?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow!("decryption failure"))
}

fn ensure_secure_parent_dir(parent: Option<&Path>) -> Result<()> {
    let Some(parent) = parent else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "MCP oauth token store: failed to create '{}'",
            parent.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "MCP oauth token store: failed to secure '{}'",
                    parent.display()
                )
            },
        )?;
    }
    Ok(())
}

fn set_secure_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).with_context(
            || {
                format!(
                    "MCP oauth token store: failed to secure '{}'",
                    path.display()
                )
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::TokenStoreConfig;

    fn test_store(base_dir: &Path, key_env: &str) -> TokenStoreConfig {
        TokenStoreConfig::EncryptedFile {
            key_env: key_env.to_string(),
            path: Some(base_dir.display().to_string()),
        }
    }

    fn test_token() -> StoredOAuthToken {
        StoredOAuthToken {
            access_token: "access-123".to_string(),
            refresh_token: Some("refresh-abc".to_string()),
            token_type: "Bearer".to_string(),
            expires_at_unix: Some(9999999999),
            scope: Some("read write".to_string()),
        }
    }

    fn setup_key(key_env: &str, key: [u8; 32]) {
        std::env::set_var(key_env, base64_encode(key));
    }

    #[test]
    fn encrypted_file_round_trip() {
        setup_key("MCP_TEST_STORE_KEY_ROUNDTRIP", [7u8; 32]);
        let base = std::env::temp_dir().join(format!("fiochat-mcp-store-{}", Uuid::new_v4()));
        let store = test_store(&base, "MCP_TEST_STORE_KEY_ROUNDTRIP");
        let token = test_token();
        save_token("linear", &store, &token).unwrap();
        let loaded = load_token("linear", &store).unwrap().unwrap();
        assert_eq!(loaded, token);
        let deleted = delete_token("linear", &store).unwrap();
        assert!(deleted);
        assert!(load_token("linear", &store).unwrap().is_none());
    }

    #[test]
    fn wrong_key_fails() {
        setup_key("MCP_TEST_STORE_KEY_WRONGKEY", [7u8; 32]);
        let base = std::env::temp_dir().join(format!("fiochat-mcp-store-{}", Uuid::new_v4()));
        let store = test_store(&base, "MCP_TEST_STORE_KEY_WRONGKEY");
        save_token("linear", &store, &test_token()).unwrap();
        std::env::set_var("MCP_TEST_STORE_KEY_WRONGKEY", base64_encode([9u8; 32]));
        let err = load_token("linear", &store).unwrap_err().to_string();
        assert!(err.contains("decrypt"), "unexpected error: {err}");
    }

    #[test]
    fn corrupt_payload_fails() {
        setup_key("MCP_TEST_STORE_KEY_CORRUPT", [7u8; 32]);
        let base = std::env::temp_dir().join(format!("fiochat-mcp-store-{}", Uuid::new_v4()));
        let store = test_store(&base, "MCP_TEST_STORE_KEY_CORRUPT");
        save_token("linear", &store, &test_token()).unwrap();
        let path = token_file_path("linear", &store).unwrap();
        std::fs::write(path, "{not-json").unwrap();
        let err = load_token("linear", &store).unwrap_err().to_string();
        assert!(err.contains("invalid payload"), "unexpected error: {err}");
    }
}
