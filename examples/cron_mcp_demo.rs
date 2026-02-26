use std::collections::HashMap;
use std::sync::Arc;

/// Demo for the cron-mcp typed wrapper.
///
/// Run with:
///   cargo run --features mcp-integrations --example cron_mcp_demo
///
/// Make sure you update the `args` path to your built cron-mcp server.
#[tokio::main]
async fn main() {
    let manager = Arc::new(aichat::mcp::McpManager::new());

    let cfg = aichat::mcp::McpServerConfig {
        name: "cron".to_string(),
        command: "node".to_string(),
        args: vec!["/path/to/cron-mcp/dist/index.js".to_string()],
        env: HashMap::new(),
        enabled: true,
        trusted: false,
        description: Some("cron-mcp demo server".to_string()),
    };

    manager.initialize(vec![cfg]).await.expect("initialize");
    manager.connect("cron").await.expect("connect");

    let cron = aichat::mcp::integrations::CronMcpClient::new(manager.clone(), "cron");

    let jobs = cron.list_jobs(None, None).await.expect("list_jobs");
    println!("{} cron jobs", jobs.len());
    for job in jobs {
        println!(
            "- [{}] {} => {}",
            if job.enabled { "on" } else { "off" },
            job.schedule,
            job.command
        );
    }
}
