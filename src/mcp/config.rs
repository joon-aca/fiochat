use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Transport kind inferred from config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Local child-process server communicating over stdio.
    Stdio,
    /// Remote server communicating over Streamable HTTP.
    Http,
}

/// Authentication configuration for remote MCP servers.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpAuthConfig {
    /// Bearer token loaded from an environment variable at connect time.
    BearerToken {
        /// Name of the environment variable containing the token.
        token_env: String,
    },
    /// OAuth 2.0 configuration (device code flow only in v1).
    #[serde(rename = "oauth")]
    OAuth {
        #[serde(flatten)]
        config: OAuthConfig,
    },
}

impl McpAuthConfig {
    /// Resolve a bearer-token auth token from env.
    pub fn resolve_bearer_token(&self) -> Result<String> {
        match self {
            McpAuthConfig::BearerToken { token_env } => std::env::var(token_env).map_err(|_| {
                anyhow::anyhow!(
                    "MCP auth: environment variable '{}' is not set or not valid UTF-8",
                    token_env
                )
            }),
            McpAuthConfig::OAuth { .. } => {
                bail!("MCP auth: oauth config cannot be resolved as a static bearer token")
            }
        }
    }

    pub fn oauth_config(&self) -> Option<&OAuthConfig> {
        match self {
            McpAuthConfig::OAuth { config } => Some(config),
            McpAuthConfig::BearerToken { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpOauthMode {
    DeviceCode,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TokenStoreConfig {
    EncryptedFile {
        /// Name of env var containing a base64-encoded 32-byte encryption key.
        key_env: String,
        /// Optional token-store path override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
}

impl TokenStoreConfig {
    pub fn validate(&self) -> Result<()> {
        match self {
            TokenStoreConfig::EncryptedFile { key_env, .. } => {
                if key_env.trim().is_empty() {
                    bail!("MCP oauth token store: 'key_env' must not be empty");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OAuthConfig {
    pub mode: McpOauthMode,
    pub client_id_env: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_env: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub device_authorization_url: String,
    pub token_url: String,
    pub token_store: TokenStoreConfig,
}

impl OAuthConfig {
    pub fn validate(&self) -> Result<()> {
        if self.client_id_env.trim().is_empty() {
            bail!("MCP oauth: 'client_id_env' must not be empty");
        }
        if let Some(secret_env) = &self.client_secret_env {
            if secret_env.trim().is_empty() {
                bail!("MCP oauth: 'client_secret_env' must not be empty when provided");
            }
        }
        validate_http_url(
            "MCP oauth: 'device_authorization_url'",
            &self.device_authorization_url,
        )?;
        validate_http_url("MCP oauth: 'token_url'", &self.token_url)?;
        self.token_store.validate()?;
        Ok(())
    }
}

/// Configuration for an MCP server.
///
/// Supports two transport modes, inferred from which fields are present:
/// - **Stdio**: set `command` (and optionally `args`, `env`) to spawn a local child process.
/// - **HTTP**: set `url` to connect to a remote Streamable HTTP server.
///
/// Exactly one of `command` or `url` must be provided.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Unique name for this server.
    pub name: String,

    /// Command to execute to start a local MCP server (stdio transport).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Arguments to pass to the command (stdio transport only).
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables to set for the server process (stdio transport only).
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    /// URL of a remote MCP server (Streamable HTTP transport).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Authentication configuration for remote servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<McpAuthConfig>,

    /// Whether this server is enabled (auto-connected at startup).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether this server is trusted (bypasses tool permission checks).
    #[serde(default)]
    pub trusted: bool,

    /// Optional description of what this server provides.
    #[serde(default)]
    pub description: Option<String>,
}

impl McpServerConfig {
    /// Determine the transport kind from the config fields.
    pub fn transport_kind(&self) -> TransportKind {
        if self.url.is_some() {
            TransportKind::Http
        } else {
            TransportKind::Stdio
        }
    }

    /// Validate config consistency. Returns an error for invalid combinations.
    pub fn validate(&self) -> Result<()> {
        let has_command = self.command.is_some();
        let has_url = self.url.is_some();

        if has_command && has_url {
            bail!(
                "MCP server '{}': cannot specify both 'command' and 'url'. \
                 Use 'command' for local stdio servers or 'url' for remote HTTP servers.",
                self.name
            );
        }

        if !has_command && !has_url {
            bail!(
                "MCP server '{}': must specify either 'command' (for stdio transport) \
                 or 'url' (for HTTP transport).",
                self.name
            );
        }

        if has_command {
            if self.auth.is_some() {
                bail!(
                    "MCP server '{}': 'auth' is only supported for HTTP transport (use 'url' instead of 'command').",
                    self.name
                );
            }
        }

        if let Some(auth) = &self.auth {
            match auth {
                McpAuthConfig::BearerToken { token_env } => {
                    if token_env.trim().is_empty() {
                        bail!("MCP server '{}': 'token_env' must not be empty", self.name);
                    }
                }
                McpAuthConfig::OAuth { config } => {
                    if !has_url {
                        bail!(
                            "MCP server '{}': oauth auth requires HTTP transport ('url').",
                            self.name
                        );
                    }
                    config.validate().map_err(|e| {
                        anyhow::anyhow!("MCP server '{}': invalid oauth config: {}", self.name, e)
                    })?;
                }
            }
        }

        Ok(())
    }
}

fn validate_http_url(field: &str, value: &str) -> Result<()> {
    let value = value.trim();
    if value.starts_with("https://") || value.starts_with("http://") {
        Ok(())
    } else {
        bail!("{field} must start with 'https://' or 'http://'");
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_config_round_trip() {
        let config = McpServerConfig {
            name: "test_server".to_string(),
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env: [("KEY".to_string(), "VALUE".to_string())].into(),
            url: None,
            auth: None,
            enabled: true,
            trusted: false,
            description: Some("A test server".to_string()),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deser: McpServerConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.name, deser.name);
        assert_eq!(config.command, deser.command);
        assert_eq!(config.args, deser.args);
        assert_eq!(config.env, deser.env);
        assert_eq!(config.enabled, deser.enabled);
        assert_eq!(config.trusted, deser.trusted);
        assert_eq!(config.description, deser.description);
        assert_eq!(deser.transport_kind(), TransportKind::Stdio);
    }

    #[test]
    fn http_config_round_trip() {
        let yaml = r#"
name: linear
url: "https://mcp.linear.app/mcp"
auth:
  type: bearer_token
  token_env: LINEAR_API_KEY
enabled: true
description: "Linear issue tracker"
"#;
        let config: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "linear");
        assert_eq!(config.url.as_deref(), Some("https://mcp.linear.app/mcp"));
        assert!(config.command.is_none());
        assert_eq!(
            config.auth,
            Some(McpAuthConfig::BearerToken {
                token_env: "LINEAR_API_KEY".to_string()
            })
        );
        assert_eq!(config.transport_kind(), TransportKind::Http);
        config.validate().unwrap();
    }

    #[test]
    fn oauth_config_round_trip() {
        let yaml = r#"
name: linear
url: "https://mcp.linear.app/mcp"
auth:
  type: oauth
  mode: device_code
  client_id_env: LINEAR_CLIENT_ID
  client_secret_env: LINEAR_CLIENT_SECRET
  scopes: ["read", "write"]
  device_authorization_url: "https://linear.app/oauth/device"
  token_url: "https://api.linear.app/oauth/token"
  token_store:
    type: encrypted_file
    key_env: FIOCHAT_MCP_TOKEN_STORE_KEY
    path: "~/.config/fiochat/secrets/mcp-oauth"
enabled: true
"#;
        let config: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        let auth = config.auth.as_ref().unwrap();
        let oauth = auth.oauth_config().unwrap();
        assert_eq!(oauth.mode, McpOauthMode::DeviceCode);
        assert_eq!(oauth.client_id_env, "LINEAR_CLIENT_ID");
        assert_eq!(
            oauth.client_secret_env.as_deref(),
            Some("LINEAR_CLIENT_SECRET")
        );
        assert_eq!(oauth.scopes, vec!["read".to_string(), "write".to_string()]);
        assert_eq!(
            oauth.device_authorization_url,
            "https://linear.app/oauth/device"
        );
        assert_eq!(oauth.token_url, "https://api.linear.app/oauth/token");
        assert_eq!(
            oauth.token_store,
            TokenStoreConfig::EncryptedFile {
                key_env: "FIOCHAT_MCP_TOKEN_STORE_KEY".to_string(),
                path: Some("~/.config/fiochat/secrets/mcp-oauth".to_string()),
            }
        );
        config.validate().unwrap();
    }

    #[test]
    fn http_config_no_auth() {
        let yaml = r#"
name: public_server
url: "https://example.com/mcp"
"#;
        let config: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.transport_kind(), TransportKind::Http);
        assert!(config.auth.is_none());
        config.validate().unwrap();
    }

    #[test]
    fn validate_rejects_both_command_and_url() {
        let config = McpServerConfig {
            name: "bad".to_string(),
            command: Some("node".to_string()),
            url: Some("https://example.com/mcp".to_string()),
            args: vec![],
            env: Default::default(),
            auth: None,
            enabled: true,
            trusted: false,
            description: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_neither_command_nor_url() {
        let config = McpServerConfig {
            name: "empty".to_string(),
            command: None,
            url: None,
            args: vec![],
            env: Default::default(),
            auth: None,
            enabled: true,
            trusted: false,
            description: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_auth_on_stdio() {
        let config = McpServerConfig {
            name: "stdio_with_auth".to_string(),
            command: Some("node".to_string()),
            url: None,
            args: vec![],
            env: Default::default(),
            auth: Some(McpAuthConfig::BearerToken {
                token_env: "TOKEN".to_string(),
            }),
            enabled: true,
            trusted: false,
            description: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_oauth_fields() {
        let config = McpServerConfig {
            name: "oauth_bad".to_string(),
            command: None,
            url: Some("https://example.com/mcp".to_string()),
            args: vec![],
            env: Default::default(),
            auth: Some(McpAuthConfig::OAuth {
                config: OAuthConfig {
                    mode: McpOauthMode::DeviceCode,
                    client_id_env: "".to_string(),
                    client_secret_env: Some("".to_string()),
                    scopes: vec![],
                    device_authorization_url: "not-a-url".to_string(),
                    token_url: "still-not-url".to_string(),
                    token_store: TokenStoreConfig::EncryptedFile {
                        key_env: "".to_string(),
                        path: None,
                    },
                },
            }),
            enabled: true,
            trusted: false,
            description: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn backward_compat_stdio_yaml() {
        let yaml = r#"
name: filesystem
command: npx
args:
  - "-y"
  - "@modelcontextprotocol/server-filesystem"
  - "/tmp"
enabled: true
trusted: false
description: "File system operations"
"#;
        let config: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.command.as_deref(), Some("npx"));
        assert!(config.url.is_none());
        assert_eq!(config.transport_kind(), TransportKind::Stdio);
        config.validate().unwrap();
    }
}
