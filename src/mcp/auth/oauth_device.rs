use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use tokio::time::{sleep, Duration};

use crate::mcp::auth::types::{DeviceCodeStart, StoredOAuthToken};
use crate::mcp::config::OAuthConfig;

pub async fn start_device_code(
    client: &reqwest::Client,
    oauth: &OAuthConfig,
) -> Result<DeviceCodeStart> {
    let (client_id, client_secret) = load_client_credentials(oauth)?;
    let mut form = vec![("client_id".to_string(), client_id)];
    if let Some(client_secret) = client_secret {
        form.push(("client_secret".to_string(), client_secret));
    }
    if !oauth.scopes.is_empty() {
        form.push(("scope".to_string(), oauth.scopes.join(" ")));
    }
    let value = post_form_json(client, &oauth.device_authorization_url, &form).await?;
    if value.get("error").is_some() {
        bail!(format_oauth_error("device authorization", &value));
    }
    let device_code = as_required_string(&value, "device_code")?;
    let user_code = as_required_string(&value, "user_code")?;
    let verification_uri = as_required_string(&value, "verification_uri")
        .or_else(|_| as_required_string(&value, "verification_url"))?;
    let verification_uri_complete = as_optional_string(&value, "verification_uri_complete");
    let expires_in = as_required_i64(&value, "expires_in")?;
    let interval = as_optional_i64(&value, "interval").unwrap_or(5).max(0);
    Ok(DeviceCodeStart {
        device_code,
        user_code,
        verification_uri,
        verification_uri_complete,
        expires_in,
        interval,
    })
}

pub async fn poll_for_token(
    client: &reqwest::Client,
    oauth: &OAuthConfig,
    start: &DeviceCodeStart,
) -> Result<StoredOAuthToken> {
    let (client_id, client_secret) = load_client_credentials(oauth)?;
    let mut interval_secs = start.interval.max(0);
    let started_at = Utc::now().timestamp();
    let expires_at = started_at + start.expires_in.max(1);
    loop {
        if Utc::now().timestamp() >= expires_at {
            bail!("MCP oauth login timed out before authorization was completed");
        }
        if interval_secs > 0 {
            sleep(Duration::from_secs(interval_secs as u64)).await;
        }

        let mut form = vec![
            (
                "grant_type".to_string(),
                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            ),
            ("device_code".to_string(), start.device_code.clone()),
            ("client_id".to_string(), client_id.clone()),
        ];
        if let Some(client_secret) = &client_secret {
            form.push(("client_secret".to_string(), client_secret.clone()));
        }

        let value = post_form_json(client, &oauth.token_url, &form).await?;
        if value.get("access_token").is_some() {
            return parse_token_response(&value, None);
        }
        let error = as_optional_string(&value, "error").unwrap_or_default();
        match error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval_secs = (interval_secs + 5).max(1);
            }
            "expired_token" => {
                bail!("MCP oauth login expired; run '/mcp auth login <server>' again")
            }
            "access_denied" => bail!("MCP oauth login denied by user"),
            _ => bail!(format_oauth_error("device token polling", &value)),
        }
    }
}

pub async fn refresh_access_token(
    client: &reqwest::Client,
    oauth: &OAuthConfig,
    refresh_token: &str,
) -> Result<StoredOAuthToken> {
    let (client_id, client_secret) = load_client_credentials(oauth)?;
    if refresh_token.trim().is_empty() {
        bail!("MCP oauth refresh token is empty");
    }
    let mut form = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("refresh_token".to_string(), refresh_token.to_string()),
        ("client_id".to_string(), client_id),
    ];
    if let Some(client_secret) = client_secret {
        form.push(("client_secret".to_string(), client_secret));
    }
    let value = post_form_json(client, &oauth.token_url, &form).await?;
    if value.get("error").is_some() {
        bail!(format_oauth_error("refresh token", &value));
    }
    parse_token_response(&value, Some(refresh_token))
}

fn load_client_credentials(oauth: &OAuthConfig) -> Result<(String, Option<String>)> {
    let client_id = std::env::var(&oauth.client_id_env).map_err(|_| {
        anyhow!(
            "MCP oauth: environment variable '{}' is not set for client_id",
            oauth.client_id_env
        )
    })?;
    let client_secret = oauth
        .client_secret_env
        .as_ref()
        .map(|key| {
            std::env::var(key).map_err(|_| {
                anyhow!(
                    "MCP oauth: environment variable '{}' is not set for client_secret",
                    key
                )
            })
        })
        .transpose()?;
    Ok((client_id, client_secret))
}

fn parse_token_response(
    value: &Value,
    fallback_refresh_token: Option<&str>,
) -> Result<StoredOAuthToken> {
    let access_token = as_required_string(value, "access_token")?;
    let refresh_token = as_optional_string(value, "refresh_token")
        .or_else(|| fallback_refresh_token.map(|v| v.to_string()));
    let token_type =
        as_optional_string(value, "token_type").unwrap_or_else(|| "Bearer".to_string());
    let scope = as_optional_string(value, "scope");
    let expires_at_unix = as_optional_i64(value, "expires_in").map(|expires_in| {
        let now = Utc::now().timestamp();
        now + expires_in.max(0)
    });
    Ok(StoredOAuthToken {
        access_token,
        refresh_token,
        token_type,
        expires_at_unix,
        scope,
    })
}

async fn post_form_json(
    client: &reqwest::Client,
    url: &str,
    form: &[(String, String)],
) -> Result<Value> {
    let response = client
        .post(url)
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .with_context(|| format!("MCP oauth request failed: {}", safe_url(url)))?;
    let value = response
        .json::<Value>()
        .await
        .with_context(|| format!("MCP oauth response is not valid JSON: {}", safe_url(url)))?;
    Ok(value)
}

fn safe_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn as_required_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| anyhow!("MCP oauth response missing required field '{}'", key))
}

fn as_optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

fn as_required_i64(value: &Value, key: &str) -> Result<i64> {
    value.get(key).and_then(|v| v.as_i64()).ok_or_else(|| {
        anyhow!(
            "MCP oauth response missing required numeric field '{}'",
            key
        )
    })
}

fn as_optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| v.as_i64())
}

fn format_oauth_error(operation: &str, value: &Value) -> String {
    let error = as_optional_string(value, "error").unwrap_or_else(|| "unknown_error".to_string());
    let description = as_optional_string(value, "error_description").unwrap_or_default();
    if description.is_empty() {
        format!("MCP oauth {} failed: {}", operation, error)
    } else {
        format!(
            "MCP oauth {} failed: {} ({})",
            operation, error, description
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::{McpOauthMode, OAuthConfig, TokenStoreConfig};
    use mockito::Matcher;

    fn oauth_config(base: &str) -> OAuthConfig {
        OAuthConfig {
            mode: McpOauthMode::DeviceCode,
            client_id_env: "MCP_TEST_CLIENT_ID".to_string(),
            client_secret_env: Some("MCP_TEST_CLIENT_SECRET".to_string()),
            scopes: vec!["read".to_string(), "write".to_string()],
            device_authorization_url: format!("{}/device", base),
            token_url: format!("{}/token", base),
            token_store: TokenStoreConfig::EncryptedFile {
                key_env: "MCP_TEST_KEY".to_string(),
                path: None,
            },
        }
    }

    #[tokio::test]
    async fn start_device_code_success() {
        std::env::set_var("MCP_TEST_CLIENT_ID", "client-1");
        std::env::set_var("MCP_TEST_CLIENT_SECRET", "secret-1");
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/device")
            .with_status(200)
            .with_body(
                r#"{
                    "device_code":"dev-123",
                    "user_code":"ABCD-EFGH",
                    "verification_uri":"https://linear.app/activate",
                    "verification_uri_complete":"https://linear.app/activate?code=ABCD-EFGH",
                    "expires_in":600,
                    "interval":0
                }"#,
            )
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let start = start_device_code(&client, &oauth_config(&server.url()))
            .await
            .unwrap();
        assert_eq!(start.device_code, "dev-123");
        assert_eq!(start.user_code, "ABCD-EFGH");
        assert_eq!(start.interval, 0);
    }

    #[tokio::test]
    async fn poll_pending_then_success() {
        std::env::set_var("MCP_TEST_CLIENT_ID", "client-1");
        std::env::set_var("MCP_TEST_CLIENT_SECRET", "secret-1");
        let mut server = mockito::Server::new_async().await;
        let _pending = server
            .mock("POST", "/token")
            .match_body(Matcher::UrlEncoded(
                "grant_type".into(),
                "urn:ietf:params:oauth:grant-type:device_code".into(),
            ))
            .with_status(400)
            .with_body(r#"{"error":"authorization_pending"}"#)
            .expect(1)
            .create_async()
            .await;
        let _ok = server
            .mock("POST", "/token")
            .with_status(200)
            .with_body(
                r#"{
                    "access_token":"access-1",
                    "refresh_token":"refresh-1",
                    "token_type":"Bearer",
                    "expires_in":3600,
                    "scope":"read write"
                }"#,
            )
            .expect(1)
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let oauth = oauth_config(&server.url());
        let start = DeviceCodeStart {
            device_code: "dev-123".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://example.com/activate".to_string(),
            verification_uri_complete: None,
            expires_in: 120,
            interval: 0,
        };
        let token = poll_for_token(&client, &oauth, &start).await.unwrap();
        assert_eq!(token.access_token, "access-1");
        assert_eq!(token.refresh_token.as_deref(), Some("refresh-1"));
    }

    #[tokio::test]
    async fn refresh_success_keeps_fallback_refresh_token() {
        std::env::set_var("MCP_TEST_CLIENT_ID", "client-1");
        std::env::set_var("MCP_TEST_CLIENT_SECRET", "secret-1");
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/token")
            .match_body(Matcher::AllOf(vec![
                Matcher::UrlEncoded("grant_type".into(), "refresh_token".into()),
                Matcher::UrlEncoded("refresh_token".into(), "refresh-legacy".into()),
            ]))
            .with_status(200)
            .with_body(
                r#"{
                    "access_token":"new-access",
                    "token_type":"Bearer",
                    "expires_in":1200
                }"#,
            )
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let token = refresh_access_token(&client, &oauth_config(&server.url()), "refresh-legacy")
            .await
            .unwrap();
        assert_eq!(token.access_token, "new-access");
        assert_eq!(token.refresh_token.as_deref(), Some("refresh-legacy"));
    }
}
