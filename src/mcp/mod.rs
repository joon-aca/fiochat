//! Model Context Protocol (MCP) client integration.
//!
//! This module lets fiochat connect to MCP servers and expose their tools via the
//! existing function-calling interface.

mod client;
mod config;
mod convert;
#[cfg(feature = "mcp-integrations")]
pub mod integrations;

pub use client::McpManager;
pub use config::McpServerConfig;

/// Check if a tool name is an MCP tool (starts with `mcp__`).
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp__")
}

/// Extract the server name from an MCP tool name.
///
/// Format: `mcp__<server_name>__<tool_name>`
pub fn extract_server_name(tool_name: &str) -> Option<String> {
    let without_prefix = tool_name.strip_prefix("mcp__")?;
    let (server, tool) = without_prefix.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some(server.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpServerConfig;

    #[test]
    fn test_is_mcp_tool() {
        assert!(is_mcp_tool("mcp__filesystem__read_file"));
        assert!(!is_mcp_tool("fs_cat"));
    }

    #[test]
    fn test_extract_server_name() {
        assert_eq!(
            extract_server_name("mcp__filesystem__read_file"),
            Some("filesystem".to_string())
        );
        assert_eq!(
            extract_server_name("mcp__my_server__tool_name"),
            Some("my_server".to_string())
        );
        assert_eq!(extract_server_name("fs_cat"), None);
        assert_eq!(extract_server_name("mcp__"), None);
        assert_eq!(extract_server_name("mcp____tool"), None);
        assert_eq!(extract_server_name("mcp__server__"), None);
    }

    #[test]
    fn test_mcp_server_config_serialization() {
        let config = McpServerConfig {
            name: "test_server".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            env: [("KEY".to_string(), "VALUE".to_string())].into(),
            enabled: true,
            trusted: false,
            description: Some("A test server".to_string()),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: McpServerConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.command, deserialized.command);
        assert_eq!(config.args, deserialized.args);
        assert_eq!(config.env, deserialized.env);
        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.trusted, deserialized.trusted);
        assert_eq!(config.description, deserialized.description);
    }
}


