# Remote OAuth for MCP (Design Note)

This document explains how first-class OAuth should work when `fiochat` runs on a remote host (for example over SSH), while browser interaction happens on your local machine.

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

## Suggested config shape for first-class OAuth

Current production path is bearer token auth (`type: bearer_token`).  
For OAuth-capable MCP servers, config could be extended as:

```yaml
mcp_servers:
  - name: linear
    url: "https://mcp.linear.app/mcp"
    auth:
      type: oauth
      mode: device_code        # or auth_code
      client_id_env: LINEAR_CLIENT_ID
      client_secret_env: LINEAR_CLIENT_SECRET   # optional for public clients
      scopes: ["read", "write"]
      token_store: "keyring"   # or encrypted_file
    enabled: true
```

For `auth_code` mode, add:

- `redirect_uri`
- optional `listen_host` / `listen_port`

## Token storage requirements

Do not store OAuth access or refresh tokens in:

- `config.yaml`
- prompt history or session memory

Use one of:

- OS keyring
- encrypted token file in `~/.config/fiochat/secrets/` (strict permissions)
- external secret manager (Vault, SSM, etc.)

Runtime should also support:

- automatic refresh on expiry
- `/mcp auth status <server>`
- `/mcp auth login <server>`
- `/mcp auth logout <server>`

## Recommended implementation order

1. implement device code OAuth first
2. keep bearer token path as fallback
3. add REPL auth-state commands
4. add pluggable secure token store

This gives remote-server-first usability without requiring browser-on-server assumptions.
