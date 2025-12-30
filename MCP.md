# MCP (Model Context Protocol) Integration

`fiochat` supports the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), allowing it to connect to external tool servers and expose their tools via the existing function-calling interface.

## Configuration

Add MCP servers to your `config.yaml`:

```yaml
mcp_servers:
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    enabled: true
    trusted: false
    description: "File system operations"
```

Tool names are exposed as:

- Format: `mcp__<server>__<tool>`
- Example: `mcp__filesystem__read_file`

## REPL Commands

Manage MCP servers in REPL:

```text
.mcp list
.mcp connect <server>
.mcp disconnect <server>
.mcp tools [server]
```

## Tool Calling Permissions

You can control tool execution globally (and override per role/session/agent):

```yaml
tool_call_permission: always  # always|ask|never
verbose_tool_calls: false
tool_permissions:
  allowed:
    - mcp__*__read_*
  denied:
    - mcp__*__delete_*
  ask:
    - mcp__*__write_*
```

If an MCP server is configured with `trusted: true`, all tools from that server bypass permission checks.


