# MCP Integrations

FioChat supports MCP in two layers:

1) **Core MCP client support** (always available)
   - Connects to MCP servers
   - Discovers tools
   - Routes tool calls from chat/agents to MCP servers
   - Lives under `fiochat::mcp`

2) **Server-specific typed wrappers** (optional)
   - Strongly-typed wrappers for specific MCP servers (example: `cron-mcp`)
   - Gated behind the `mcp-integrations` cargo feature

For practical setup instructions (including Linear), see:

- [`docs/mcp-setup.md`](./mcp-setup.md)
- [`docs/mcp-remote-oauth.md`](./mcp-remote-oauth.md) for first-class remote OAuth design guidance

## Transport support

Core MCP now supports two connection modes:

- **stdio transport** (local process): configure `command` and optional `args`/`env`
- **HTTP transport** (remote Streamable HTTP): configure `url` and optional `auth` (`bearer_token` or `oauth`)

This enables remote MCP servers (for example Linear) without writing per-server Rust code.

## Config shape

MCP servers are configured in `config.yaml` under `mcp_servers`.

### Local stdio example

```yaml
mcp_servers:
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    enabled: true
    trusted: false
    description: "File system operations"
```

### Remote HTTP example (Linear-style)

```yaml
mcp_servers:
  - name: linear
    url: "https://mcp.linear.app/mcp"
    auth:
      type: bearer_token
      token_env: LINEAR_API_KEY
    enabled: true
    trusted: false
    description: "Linear issue tracker"
```

### Remote HTTP OAuth example (device code mode)

```yaml
mcp_servers:
  - name: linear-oauth
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
    trusted: false
    description: "Linear issue tracker (OAuth)"
```

Notes:

- Exactly one of `command` or `url` must be set
- `auth` is currently for HTTP transport
- Bearer token values should live in `.env`, never directly in `config.yaml`
- OAuth device-code tokens are stored encrypted in the configured token store
- OAuth for remote-host workflows is documented in `docs/mcp-remote-oauth.md`

## Runtime behavior

At startup, FioChat:

1. loads `config.yaml` and `.env`
2. validates each MCP server config
3. initializes valid MCP servers
4. auto-connects `enabled: true` servers (non-fatal on individual failures)
5. resolves HTTP auth:
   - bearer token from env, or
   - OAuth token from encrypted store with refresh-on-expiry
6. exposes discovered MCP tools to function-calling as `mcp__<server>__<tool>`

In REPL, use:

```text
/mcp list
/mcp connect <server>
/mcp disconnect <server>
/mcp tools [server]
/mcp auth status <server>
/mcp auth login <server>
/mcp auth logout <server>
```

## Terminology

- **MCP server**: any tool provider speaking MCP (local stdio process or remote HTTP endpoint)
- **Core MCP support**: transport, connection lifecycle, tool discovery, call routing
- **MCP integration**: typed Rust wrapper over a specific server’s tool contract

## Optional typed wrappers (`mcp-integrations` feature)

Build/run with:

```bash
cargo build --features mcp-integrations
```

Current example wrapper:

- Module: `fiochat::mcp::integrations::CronMcpClient`
- Demo: `cargo run --features mcp-integrations --example cron_mcp_demo`

Use typed wrappers when you need compile-time contracts and ergonomic Rust APIs. Use core MCP-only config when you want rapid server onboarding with no new Rust code.
