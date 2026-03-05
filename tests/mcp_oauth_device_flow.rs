use fiochat::base64_encode;
use fiochat::mcp::auth::{
    oauth_login_complete, oauth_login_start, oauth_status, resolve_http_auth_header,
    OAuthStatusKind,
};
use fiochat::mcp::{McpAuthConfig, McpOauthMode, OAuthConfig, TokenStoreConfig};
use mockito::Matcher;
use uuid::Uuid;

fn oauth_config(base_url: &str, path: &str) -> OAuthConfig {
    OAuthConfig {
        mode: McpOauthMode::DeviceCode,
        client_id_env: "MCP_IT_CLIENT_ID".to_string(),
        client_secret_env: Some("MCP_IT_CLIENT_SECRET".to_string()),
        scopes: vec!["read".to_string()],
        device_authorization_url: format!("{}/device", base_url),
        token_url: format!("{}/token", base_url),
        token_store: TokenStoreConfig::EncryptedFile {
            key_env: "MCP_IT_TOKEN_STORE_KEY".to_string(),
            path: Some(path.to_string()),
        },
    }
}

#[tokio::test]
async fn oauth_device_flow_persists_and_resolves_token() {
    std::env::set_var("MCP_IT_CLIENT_ID", "client-id");
    std::env::set_var("MCP_IT_CLIENT_SECRET", "client-secret");
    std::env::set_var("MCP_IT_TOKEN_STORE_KEY", base64_encode([11u8; 32]));

    let mut server = mockito::Server::new_async().await;
    let _device = server
        .mock("POST", "/device")
        .match_body(Matcher::UrlEncoded("client_id".into(), "client-id".into()))
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
    let _token = server
        .mock("POST", "/token")
        .match_body(Matcher::UrlEncoded(
            "grant_type".into(),
            "urn:ietf:params:oauth:grant-type:device_code".into(),
        ))
        .with_status(200)
        .with_body(
            r#"{
              "access_token":"access-login",
              "refresh_token":"refresh-login",
              "token_type":"Bearer",
              "expires_in":3600
            }"#,
        )
        .expect(1)
        .create_async()
        .await;

    let token_path = std::env::temp_dir().join(format!("fiochat-mcp-it-{}", Uuid::new_v4()));
    let oauth = oauth_config(&server.url(), &token_path.display().to_string());

    let status = oauth_status("linear", &oauth).await;
    assert_eq!(status.kind, OAuthStatusKind::LoggedOut);

    let start = oauth_login_start(&oauth).await.unwrap();
    oauth_login_complete("linear", &oauth, &start)
        .await
        .unwrap();

    let status = oauth_status("linear", &oauth).await;
    assert_eq!(status.kind, OAuthStatusKind::TokenValid);

    let auth = McpAuthConfig::OAuth {
        config: oauth.clone(),
    };
    let token = resolve_http_auth_header("linear", &auth).await.unwrap();
    assert_eq!(token, "access-login");
}
