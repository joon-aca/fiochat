# MCP Integrations

This repo supports the Model Context Protocol (MCP) in two layers:

1) **Core MCP client support** (always enabled)
   - Starts MCP servers as child processes, lists tools, and calls tools.
   - This lives under the `aichat::mcp` module.

2) **MCP server-specific integrations** (optional)
   - Typed wrappers for a specific MCP server’s tools (e.g., `cron-mcp`).
   - These are *not* part of the core project surface and are gated behind the `mcp-integrations` feature.

## Terminology

- **MCP server**: a separate process that exposes tools over MCP (stdio). `cron-mcp` is an MCP server.
- **MCP integration**: a typed wrapper/adapter in this repo that knows a specific server’s tool names and JSON contract.

## Enable integrations

Build/run with:

```bash
cargo build --features mcp-integrations
```

## cron-mcp wrapper

- Wrapper module: `aichat::mcp::integrations::CronMcpClient`
- Demo: `cargo run --features mcp-integrations --example cron_mcp_demo`
