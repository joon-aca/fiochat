# MCP Setup Guide (Practical)

This guide shows the fastest reliable way to configure MCP in FioChat today, including remote MCP servers (such as Linear) using bearer-token auth.

It is designed to be easy to follow without a first-class OAuth browser flow.

## What you are configuring

FioChat supports two MCP connection types:

- `stdio` (local process): starts a local MCP server via `command` + `args`
- `HTTP` (remote): connects to a remote MCP endpoint via `url`

For HTTP servers, auth currently uses a bearer token read from an environment variable.

## 1) Locate your FioChat config directory

Default:

- `~/.config/fiochat`

Main files:

- `~/.config/fiochat/config.yaml`
- `~/.config/fiochat/.env`

## 2) Add your token to `.env`

Edit:

- `~/.config/fiochat/.env`

Add:

```bash
LINEAR_API_KEY=your_linear_token_here
```

Guidelines:

- Use the real token value, no quotes needed
- Do not commit this file
- Do not put token values directly into `config.yaml`

## 3) Add Linear MCP server to `config.yaml`

Edit:

- `~/.config/fiochat/config.yaml`

Add this under `mcp_servers`:

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

Notes:

- `url` means remote HTTP transport
- `token_env` is the env var name, not the token itself
- `trusted: false` is recommended (keeps tool permission checks active)

## 4) Restart FioChat

Restart `fiochat` so it reloads:

- `config.yaml`
- `.env`

## 5) Verify server connection in REPL

Inside FioChat REPL:

```text
/mcp list
/mcp connect linear
/mcp tools linear
```

Expected:

- `/mcp list` shows `linear`
- `/mcp connect linear` succeeds
- `/mcp tools linear` returns available tool names

If the server is enabled, startup auto-connect will also attempt connection.

## 6) Minimum checklist before ticket generation

Before using chat-to-ticket workflows, confirm:

- Linear token works
- `linear` MCP tools are listed
- your default team key is known (for example `MWB`)
- labels and states exist in Linear as expected

## 7) Example mixed MCP config (local + remote)

```yaml
mcp_servers:
  # Local stdio MCP server
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    enabled: true
    trusted: false

  # Remote HTTP MCP server
  - name: linear
    url: "https://mcp.linear.app/mcp"
    auth:
      type: bearer_token
      token_env: LINEAR_API_KEY
    enabled: true
    trusted: false
```

## Troubleshooting

### `MCP auth: environment variable 'LINEAR_API_KEY' is not set`

- Ensure `LINEAR_API_KEY` exists in `~/.config/fiochat/.env`
- Restart FioChat after editing `.env`

### Server appears in `/mcp list` but cannot connect

- Check the `url` is correct and reachable
- Verify token validity in Linear
- Keep `trusted: false`; trust does not fix auth/connectivity issues

### No tools shown by `/mcp tools linear`

- Connection likely failed or server returned no tools
- Re-run `/mcp connect linear` and review the error message

## Current auth limitation

Current recommended auth path is bearer token via env var.

First-class OAuth browser flow is not yet integrated into FioChat runtime. When OAuth support lands, this guide can be simplified further.

For remote-host OAuth design patterns (device code, SSH-tunneled callback, token storage model), see:

- [`docs/mcp-remote-oauth.md`](./mcp-remote-oauth.md)
