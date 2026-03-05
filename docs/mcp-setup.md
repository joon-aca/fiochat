# MCP Setup Guide (Practical)

FioChat supports MCP over local `stdio` and remote `HTTP`.  
For remote HTTP servers you can use either:

- `bearer_token` auth (env var token)
- `oauth` auth (device code flow + encrypted token file store)

## 1) Locate your config directory

Default directory:

- `~/.config/fiochat`

Main files:

- `~/.config/fiochat/config.yaml`
- `~/.config/fiochat/.env`

## 2) Bearer token setup (simple)

Add to `~/.config/fiochat/.env`:

```bash
LINEAR_API_KEY=your_linear_token_here
```

Add server config:

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

## 3) OAuth device code setup (remote/headless friendly)

### 3.1 Add OAuth env vars

Edit `~/.config/fiochat/.env` and add:

```bash
LINEAR_CLIENT_ID=your_oauth_client_id
LINEAR_CLIENT_SECRET=your_oauth_client_secret
FIOCHAT_MCP_TOKEN_STORE_KEY=base64_32_byte_key
```

Generate `FIOCHAT_MCP_TOKEN_STORE_KEY` (example):

```bash
openssl rand -base64 32
```

### 3.2 Add OAuth MCP config

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
        path: "~/.config/fiochat/secrets/mcp-oauth" # optional
    enabled: true
    trusted: false
    description: "Linear issue tracker (OAuth)"
```

Notes:

- OAuth tokens are encrypted at rest in the token store.
- Tokens are never stored in `config.yaml`.
- `key_env` must decode to exactly 32 bytes (base64).

## 4) Restart fiochat

Restart after editing config/env files so values are reloaded.

## 5) Verify in REPL

Bearer flow:

```text
/mcp list
/mcp connect linear
/mcp tools linear
```

OAuth flow:

```text
/mcp auth status linear-oauth
/mcp auth login linear-oauth
/mcp auth status linear-oauth
/mcp connect linear-oauth
/mcp tools linear-oauth
```

Logout if needed:

```text
/mcp auth logout linear-oauth
```

## 6) Mixed config example (local + bearer + OAuth)

```yaml
mcp_servers:
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    enabled: true
    trusted: false

  - name: linear
    url: "https://mcp.linear.app/mcp"
    auth:
      type: bearer_token
      token_env: LINEAR_API_KEY
    enabled: true
    trusted: false

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
    enabled: false
    trusted: false
```

## Troubleshooting

### `MCP auth: environment variable 'X' is not set`

- Ensure the env var exists in `~/.config/fiochat/.env`
- Restart fiochat after editing `.env`

### `must decode to exactly 32 bytes`

- Regenerate token-store key with `openssl rand -base64 32`
- Ensure no extra spaces/newlines in the env value

### OAuth status shows `token_invalid`

- Run `/mcp auth logout <server>`
- Run `/mcp auth login <server>` again

### Connection works once then fails later

- Token may be expired and refresh failed
- Re-login via `/mcp auth login <server>`

## Related docs

- [`docs/mcp-integrations.md`](./mcp-integrations.md)
- [`docs/mcp-remote-oauth.md`](./mcp-remote-oauth.md)
