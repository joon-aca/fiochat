mod oauth_device;
mod store;
mod types;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;

use crate::mcp::config::{McpAuthConfig, OAuthConfig};
pub use types::{DeviceCodeStart, StoredOAuthToken};

const TOKEN_EXPIRY_SKEW_SECS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthStatusKind {
    LoggedOut,
    TokenValid,
    TokenExpiredRefreshable,
    TokenInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthStatus {
    pub kind: OAuthStatusKind,
    pub expires_at_unix: Option<i64>,
    pub detail: Option<String>,
}

impl OAuthStatusKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OAuthStatusKind::LoggedOut => "logged_out",
            OAuthStatusKind::TokenValid => "token_valid",
            OAuthStatusKind::TokenExpiredRefreshable => "token_expired_refreshable",
            OAuthStatusKind::TokenInvalid => "token_invalid",
        }
    }
}

pub async fn resolve_http_auth_header(
    server_name: &str,
    auth_cfg: &McpAuthConfig,
) -> Result<String> {
    match auth_cfg {
        McpAuthConfig::BearerToken { .. } => auth_cfg.resolve_bearer_token(),
        McpAuthConfig::OAuth { config } => resolve_oauth_token(server_name, config).await,
    }
}

pub async fn oauth_status(server_name: &str, oauth: &OAuthConfig) -> OAuthStatus {
    match store::load_token(server_name, &oauth.token_store) {
        Ok(None) => OAuthStatus {
            kind: OAuthStatusKind::LoggedOut,
            expires_at_unix: None,
            detail: None,
        },
        Ok(Some(token)) => {
            if token.access_token.trim().is_empty() {
                OAuthStatus {
                    kind: OAuthStatusKind::TokenInvalid,
                    expires_at_unix: token.expires_at_unix,
                    detail: Some("stored access token is empty".to_string()),
                }
            } else if is_token_valid(&token) {
                OAuthStatus {
                    kind: OAuthStatusKind::TokenValid,
                    expires_at_unix: token.expires_at_unix,
                    detail: None,
                }
            } else if token.has_refresh_token() {
                OAuthStatus {
                    kind: OAuthStatusKind::TokenExpiredRefreshable,
                    expires_at_unix: token.expires_at_unix,
                    detail: None,
                }
            } else {
                OAuthStatus {
                    kind: OAuthStatusKind::TokenInvalid,
                    expires_at_unix: token.expires_at_unix,
                    detail: Some("token expired and no refresh token is available".to_string()),
                }
            }
        }
        Err(err) => OAuthStatus {
            kind: OAuthStatusKind::TokenInvalid,
            expires_at_unix: None,
            detail: Some(err.to_string()),
        },
    }
}

pub async fn oauth_login_start(oauth: &OAuthConfig) -> Result<DeviceCodeStart> {
    let client = reqwest::Client::new();
    oauth_device::start_device_code(&client, oauth).await
}

pub async fn oauth_login_complete(
    server_name: &str,
    oauth: &OAuthConfig,
    start: &DeviceCodeStart,
) -> Result<StoredOAuthToken> {
    let client = reqwest::Client::new();
    let token = oauth_device::poll_for_token(&client, oauth, start)
        .await
        .context("MCP oauth login failed")?;
    store::save_token(server_name, &oauth.token_store, &token)?;
    Ok(token)
}

pub fn oauth_logout(server_name: &str, oauth: &OAuthConfig) -> Result<bool> {
    store::delete_token(server_name, &oauth.token_store)
}

async fn resolve_oauth_token(server_name: &str, oauth: &OAuthConfig) -> Result<String> {
    let token = match store::load_token(server_name, &oauth.token_store) {
        Ok(Some(token)) => token,
        Ok(None) => {
            bail!(
                "MCP oauth token not found for server '{}'. Run '/mcp auth login {}' first.",
                server_name,
                server_name
            );
        }
        Err(err) => {
            return Err(anyhow!(
                "MCP oauth token load failed for server '{}': {}",
                server_name,
                err
            ))
        }
    };

    if token.access_token.trim().is_empty() {
        bail!(
            "MCP oauth token for server '{}' is invalid. Run '/mcp auth login {}' again.",
            server_name,
            server_name
        );
    }
    if is_token_valid(&token) {
        return Ok(token.access_token);
    }

    let refresh_token = token.refresh_token.as_deref().ok_or_else(|| {
        anyhow!(
            "MCP oauth token for server '{}' expired and has no refresh token. Run '/mcp auth login {}' again.",
            server_name,
            server_name
        )
    })?;

    let client = reqwest::Client::new();
    let refreshed = oauth_device::refresh_access_token(&client, oauth, refresh_token)
        .await
        .with_context(|| {
            format!(
                "MCP oauth refresh failed for server '{}'; run '/mcp auth login {}' again",
                server_name, server_name
            )
        })?;
    store::save_token(server_name, &oauth.token_store, &refreshed)?;
    Ok(refreshed.access_token)
}

fn is_token_valid(token: &StoredOAuthToken) -> bool {
    if token.access_token.trim().is_empty() {
        return false;
    }
    match token.expires_at_unix {
        Some(expires_at) => {
            let now = Utc::now().timestamp();
            now < (expires_at - TOKEN_EXPIRY_SKEW_SECS)
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::{McpOauthMode, OAuthConfig, TokenStoreConfig};
    use crate::utils::base64_encode;
    use mockito::Matcher;
    use uuid::Uuid;

    fn test_oauth(base: &str, key_env: &str, path: &str) -> OAuthConfig {
        OAuthConfig {
            mode: McpOauthMode::DeviceCode,
            client_id_env: "MCP_TEST_CLIENT_ID".to_string(),
            client_secret_env: Some("MCP_TEST_CLIENT_SECRET".to_string()),
            scopes: vec!["read".to_string()],
            device_authorization_url: format!("{}/device", base),
            token_url: format!("{}/token", base),
            token_store: TokenStoreConfig::EncryptedFile {
                key_env: key_env.to_string(),
                path: Some(path.to_string()),
            },
        }
    }

    #[tokio::test]
    async fn oauth_status_reports_logged_out() {
        let path = std::env::temp_dir()
            .join(format!("fiochat-mcp-auth-{}", Uuid::new_v4()))
            .display()
            .to_string();
        std::env::set_var("MCP_TEST_STORE_KEY_STATUS", base64_encode([5u8; 32]));
        let oauth = test_oauth("https://example.com", "MCP_TEST_STORE_KEY_STATUS", &path);
        let status = oauth_status("linear", &oauth).await;
        assert_eq!(status.kind, OAuthStatusKind::LoggedOut);
    }

    #[tokio::test]
    async fn resolve_refreshes_expired_token() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/token")
            .match_body(Matcher::UrlEncoded(
                "grant_type".into(),
                "refresh_token".into(),
            ))
            .with_status(200)
            .with_body(
                r#"{
                    "access_token":"refreshed-token",
                    "refresh_token":"refresh-new",
                    "token_type":"Bearer",
                    "expires_in":3600
                }"#,
            )
            .create_async()
            .await;

        std::env::set_var("MCP_TEST_CLIENT_ID", "client-id");
        std::env::set_var("MCP_TEST_CLIENT_SECRET", "client-secret");
        std::env::set_var("MCP_TEST_STORE_KEY_REFRESH", base64_encode([1u8; 32]));

        let path = std::env::temp_dir().join(format!("fiochat-mcp-auth-{}", Uuid::new_v4()));
        let oauth = test_oauth(
            &server.url(),
            "MCP_TEST_STORE_KEY_REFRESH",
            &path.display().to_string(),
        );
        let expired = StoredOAuthToken {
            access_token: "expired".to_string(),
            refresh_token: Some("refresh-old".to_string()),
            token_type: "Bearer".to_string(),
            expires_at_unix: Some(Utc::now().timestamp() - 100),
            scope: None,
        };
        store::save_token("linear", &oauth.token_store, &expired).unwrap();

        let header_token = resolve_oauth_token("linear", &oauth).await.unwrap();
        assert_eq!(header_token, "refreshed-token");
        let loaded = store::load_token("linear", &oauth.token_store)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.access_token, "refreshed-token");
    }
}
