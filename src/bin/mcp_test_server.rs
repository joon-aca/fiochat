use rmcp::handler::server::{router::Router, tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_router, Json, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct EchoInput {
    text: String,
    #[serde(default)]
    count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct EchoOutput {
    echoed: String,
    count: Option<u32>,
}

#[derive(Clone)]
struct TestServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TestServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "echo_structured",
        description = "Echo input as structured JSON"
    )]
    async fn echo_structured(
        &self,
        params: Parameters<EchoInput>,
    ) -> Result<Json<EchoOutput>, String> {
        Ok(Json(EchoOutput {
            echoed: params.0.text,
            count: params.0.count,
        }))
    }
}

impl ServerHandler for TestServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "fiochat-mcp-test-server".into(),
                title: None,
                version: "0.1.0".into(),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Run an MCP server over stdio until the client disconnects.
    let server = TestServer::new();
    let tool_routes = server.tool_router.clone();
    let router = Router::new(server).with_tools(tool_routes);
    let running = rmcp::serve_server(router, (tokio::io::stdin(), tokio::io::stdout())).await?;
    let _ = running.waiting().await?;
    Ok(())
}
