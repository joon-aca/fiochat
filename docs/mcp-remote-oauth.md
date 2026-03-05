# Remote OAuth for MCP

This document explains remote-host OAuth behavior when `fiochat` runs on a remote box (for example over SSH), while browser interaction happens on your local machine.

## Current implementation status

- Implemented: OAuth device code flow (`mode: device_code`)
- Implemented: encrypted-file token store (`token_store.type: encrypted_file`)
- Implemented: token refresh on expiry during auth resolution
- Implemented REPL commands:
  - `/mcp auth status <server>`
  - `/mcp auth login <server>`
  - `/mcp auth logout <server>`
- Deferred: auth code callback/tunnel mode (`mode: auth_code`)

## Core issue

With remote `fiochat`:

- OAuth browser interaction happens locally
- tokens must be stored on the remote host where `fiochat` runs

A complete design therefore needs:

1. a browser-based approval path
2. a secure remote token delivery and storage path

## Practical patterns

### 1) Device code flow (recommended for remote/headless)

Best default for SSH and headless environments.

Flow:

1. user runs `/mcp auth login linear`
2. fiochat requests a device code from provider
3. fiochat prints verification URL + short code
4. user completes approval in local browser
5. fiochat polls until approved
6. fiochat stores access/refresh tokens on remote host

Why this is preferred:

- no callback listener required
- no SSH tunnel required
- reliable for remote CLI usage

### 2) Auth code + local callback via SSH tunnel

Flow:

1. fiochat starts callback listener on remote (`127.0.0.1:<port>`)
2. user opens SSH tunnel (`ssh -L <port>:127.0.0.1:<port> remote`)
3. browser redirects to `http://localhost:<port>/callback`
4. remote fiochat receives code, exchanges tokens, stores securely

This works but is more operationally fragile than device code.

### 3) Auth broker service (team/enterprise scale)

Use a central internal auth service:

- browser flow completes against broker
- broker stores and rotates long-lived credentials
- fiochat fetches short-lived tokens using service identity

Best for many users/servers, but requires additional infrastructure.

## OAuth config shape

Current bearer-token auth (`type: bearer_token`) is still supported.  
OAuth config now supports:

```yaml
mcp_servers:
  - name: linear
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
```

## Token storage requirements

Do not store OAuth access or refresh tokens in:

- `config.yaml`
- prompt history or session memory

Current backend:

- encrypted token file store with strict permissions
- encryption key from `token_store.key_env` (base64 32-byte key)

Future backends:

- OS keyring
- external secret managers (Vault, SSM, etc.)

Runtime should also support:

- automatic refresh on expiry
- `/mcp auth status <server>`
- `/mcp auth login <server>`
- `/mcp auth logout <server>`

## Future work

1. add `mode: auth_code` callback/tunnel support
2. add OS keyring token backend
3. add external secret-manager backend(s)
