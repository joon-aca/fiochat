use fiochat::base64_encode;
use fiochat::mcp::auth::{oauth_login_complete, oauth_login_start, resolve_http_auth_header};
use fiochat::mcp::{McpAuthConfig, McpOauthMode, OAuthConfig, TokenStoreConfig};
use mockito::Matcher;
use uuid::Uuid;

fn oauth_config(base_url: &str, path: &str) -> OAuthConfig {
    OAuthConfig {
        mode: McpOauthMode::DeviceCode,
        client_id_env: "MCP_REFRESH_CLIENT_ID".to_string(),
        client_secret_env: Some("MCP_REFRESH_CLIENT_SECRET".to_string()),
        scopes: vec!["read".to_string()],
        device_authorization_url: format!("{}/device", base_url),
        token_url: format!("{}/token", base_url),
        token_store: TokenStoreConfig::EncryptedFile {
            key_env: "MCP_REFRESH_STORE_KEY".to_string(),
            path: Some(path.to_string()),
        },
    }
}

#[tokio::test]
async fn resolve_http_auth_header_refreshes_expired_token() {
    std::env::set_var("MCP_REFRESH_CLIENT_ID", "client-id");
    std::env::set_var("MCP_REFRESH_CLIENT_SECRET", "client-secret");
    std::env::set_var("MCP_REFRESH_STORE_KEY", base64_encode([13u8; 32]));

    let mut server = mockito::Server::new_async().await;
    let _device = server
        .mock("POST", "/device")
        .with_status(200)
        .with_body(
            r#"{
              "device_code":"dev-code",
              "user_code":"ABCD-EFGH",
              "verification_uri":"https://example.com/activate",
              "expires_in":120,
              "interval":0
            }"#,
        )
        .expect(1)
        .create_async()
        .await;
    let _device_token = server
        .mock("POST", "/token")
        .match_body(Matcher::UrlEncoded(
            "grant_type".into(),
            "urn:ietf:params:oauth:grant-type:device_code".into(),
        ))
        .with_status(200)
        .with_body(
            r#"{
              "access_token":"expired-immediately",
              "refresh_token":"refresh-1",
              "token_type":"Bearer",
              "expires_in":0
            }"#,
        )
        .expect(1)
        .create_async()
        .await;
    let _refresh = server
        .mock("POST", "/token")
        .match_body(Matcher::AllOf(vec![
            Matcher::UrlEncoded("grant_type".into(), "refresh_token".into()),
            Matcher::UrlEncoded("refresh_token".into(), "refresh-1".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{
              "access_token":"refreshed-access",
              "refresh_token":"refresh-2",
              "token_type":"Bearer",
              "expires_in":3600
            }"#,
        )
        .expect(1)
        .create_async()
        .await;

    let token_path = std::env::temp_dir().join(format!("fiochat-mcp-it-{}", Uuid::new_v4()));
    let oauth = oauth_config(&server.url(), &token_path.display().to_string());

    let start = oauth_login_start(&oauth).await.unwrap();
    oauth_login_complete("linear", &oauth, &start)
        .await
        .unwrap();

    let auth = McpAuthConfig::OAuth {
        config: oauth.clone(),
    };
    let token = resolve_http_auth_header("linear", &auth).await.unwrap();
    assert_eq!(token, "refreshed-access");
}
