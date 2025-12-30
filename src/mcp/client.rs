use anyhow::{anyhow, bail, Result};
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::TokioChildProcess;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::config::McpServerConfig;
use super::convert::mcp_tool_to_function;
use crate::function::FunctionDeclaration;

/// Wrapper around a single MCP server connection.
pub struct McpClient {
    name: String,
    config: McpServerConfig,
    tools: Arc<RwLock<Vec<FunctionDeclaration>>>,
    connected: Arc<RwLock<bool>>,
    service: Arc<RwLock<Option<RunningService<RoleClient, ()>>>>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("name", &self.name)
            .field("config", &self.config)
            .field("connected", &self.connected)
            .field("tools_len", &"<tools>")
            .field("service", &"<MCP Service>")
            .finish()
    }
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        let name = config.name.clone();
        Self {
            name,
            config,
            tools: Arc::new(RwLock::new(Vec::new())),
            connected: Arc::new(RwLock::new(false)),
            service: Arc::new(RwLock::new(None)),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    pub async fn connect(&self) -> Result<()> {
        if *self.connected.read().await {
            return Ok(());
        }

        log::info!("Connecting to MCP server '{}'...", self.name);

        // Create child process command.
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            anyhow!(
                "Failed to create transport for MCP server '{}': {}",
                self.name,
                e
            )
        })?;

        // rmcp requires a type that implements ServiceExt to serve the transport.
        let role = ();
        let service = role.serve(transport).await.map_err(|e| {
            anyhow!(
                "Failed to initialize MCP service for server '{}': {}",
                self.name,
                e
            )
        })?;

        log::debug!("Connected to MCP server '{}': {:?}", self.name, service.peer_info());

        // Discover tools.
        let mut discovered_tools = Vec::new();
        match service.list_tools(Default::default()).await {
            Ok(tools_result) => {
                log::info!(
                    "MCP server '{}' provided {} tools",
                    self.name,
                    tools_result.tools.len()
                );
                for tool in tools_result.tools {
                    let schema_value =
                        serde_json::to_value(&tool.input_schema).unwrap_or_else(|_| {
                            serde_json::json!({})
                        });
                    match mcp_tool_to_function(
                        &self.name,
                        &tool.name,
                        &tool.description.unwrap_or_default(),
                        &schema_value,
                    ) {
                        Ok(func_decl) => discovered_tools.push(func_decl),
                        Err(e) => log::warn!(
                            "Failed to convert MCP tool '{}' from server '{}': {}",
                            tool.name,
                            self.name,
                            e
                        ),
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to list tools from MCP server '{}': {}", self.name, e);
            }
        }

        *self.tools.write().await = discovered_tools;
        *self.service.write().await = Some(service);
        *self.connected.write().await = true;

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        if !*self.connected.read().await {
            return Ok(());
        }

        log::info!("Disconnecting from MCP server '{}'...", self.name);

        if let Some(service) = self.service.write().await.take() {
            if let Err(e) = service.cancel().await {
                log::warn!("Error during shutdown of MCP server '{}': {}", self.name, e);
            }
        }

        *self.connected.write().await = false;
        *self.tools.write().await = Vec::new();
        Ok(())
    }

    pub async fn get_tools(&self) -> Vec<FunctionDeclaration> {
        self.tools.read().await.clone()
    }

    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        if !*self.connected.read().await {
            bail!("MCP server '{}' is not connected", self.name);
        }

        let service_guard = self.service.read().await;
        let service = service_guard
            .as_ref()
            .ok_or_else(|| anyhow!("MCP service not initialized for server '{}'", self.name))?;

        let arguments_map = match arguments {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(anyhow!(
                    "Tool arguments must be a JSON object or null, got: {}",
                    other
                ))
            }
        };

        let params = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: arguments_map,
        };

        let result = service.call_tool(params).await.map_err(|e| {
            anyhow!(
                "Failed to call tool '{}' on MCP server '{}': {}",
                tool_name,
                self.name,
                e
            )
        })?;

        serde_json::to_value(&result).map_err(|e| anyhow!("Failed to serialize tool result: {}", e))
    }
}

/// Manager for MCP server connections.
#[derive(Debug, Default)]
pub struct McpManager {
    clients: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn initialize(&self, configs: Vec<McpServerConfig>) -> Result<()> {
        let mut clients = self.clients.write().await;
        for config in configs {
            let name = config.name.clone();
            clients.insert(name, Arc::new(McpClient::new(config)));
        }
        Ok(())
    }

    pub async fn connect(&self, server_name: &str) -> Result<()> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not found", server_name))?;
        client.connect().await
    }

    pub async fn connect_all(&self) -> Result<()> {
        let clients = self.clients.read().await;
        for client in clients.values() {
            if !client.config.enabled {
                continue;
            }
            if let Err(e) = client.connect().await {
                log::warn!("Failed to connect to MCP server '{}': {}", client.name(), e);
            }
        }
        Ok(())
    }

    pub async fn disconnect(&self, server_name: &str) -> Result<()> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not found", server_name))?;
        client.disconnect().await
    }

    pub async fn get_all_tools(&self) -> Vec<FunctionDeclaration> {
        let clients = self.clients.read().await;
        let mut tools = Vec::new();
        for client in clients.values() {
            if client.is_connected().await {
                tools.extend(client.get_tools().await);
            }
        }
        tools
    }

    pub async fn get_server_tools(&self, server_name: &str) -> Result<Vec<FunctionDeclaration>> {
        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not found", server_name))?;
        Ok(client.get_tools().await)
    }

    pub async fn call_tool(&self, prefixed_name: &str, arguments: Value) -> Result<Value> {
        let parts: Vec<&str> = prefixed_name
            .strip_prefix("mcp__")
            .ok_or_else(|| anyhow!("Invalid MCP tool name: {}", prefixed_name))?
            .splitn(2, "__")
            .collect();
        if parts.len() != 2 {
            bail!("Invalid MCP tool name format: {}", prefixed_name);
        }
        let server_name = parts[0];
        let tool_name = parts[1];

        let clients = self.clients.read().await;
        let client = clients
            .get(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not found", server_name))?;
        client.call_tool(tool_name, arguments).await
    }

    pub async fn list_servers(&self) -> Vec<(String, bool, Option<String>)> {
        let clients = self.clients.read().await;
        let mut servers = Vec::new();
        for (name, client) in clients.iter() {
            let connected = client.is_connected().await;
            let description = client.config.description.clone();
            servers.push((name.clone(), connected, description));
        }
        servers.sort_by(|a, b| a.0.cmp(&b.0));
        servers
    }
}


