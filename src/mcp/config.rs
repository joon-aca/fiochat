use serde::{Deserialize, Serialize};

/// Configuration for an MCP server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Unique name for this server.
    pub name: String,

    /// Command to execute to start the MCP server.
    pub command: String,

    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables to set for the server process.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

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

fn default_true() -> bool {
    true
}


