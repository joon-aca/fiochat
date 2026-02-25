# Fiochat: AI-Powered Server Operations via Telegram

> Chat with your servers. Talk to **Fio**, your AI ops steward, and manage your infrastructure through natural conversation.

Fiochat is an AI-powered DevOps bot that enables **chat-based server operations** through Telegram. You talk to Fio via server-specific bots like `capraia-ops-bot` or `gorgona-ops-bot`, and Fio interprets your commands, executes them safely, and returns structured results.

```
Telegram  →  Fio (Telegram Bot)  →  Fiochat (AI Service)  →  Server Operations
```

## Features

### Telegram Integration
- Per-server Telegram bots (`<server>-ops-bot`)
- Natural language server operations
- User authentication via Telegram user IDs
- Session persistence across conversations
- Real-time responses with thinking indicators

### AI Service (built on [AIChat](https://github.com/sigoden/aichat))
- **Multi-Provider Support**: OpenAI, Claude, Azure-OpenAI, Gemini, Ollama, and 20+ more
- **MCP Support**: Model Context Protocol for tool integration
- **Function Calling**: Connect to external tools and data sources
- **RAG**: Integrate external documents for contextual responses
- **HTTP Server**: OpenAI-compatible API endpoint

### Server Operations
- System info and status
- Service management
- Log tailing
- Script execution
- Cron job management
- Health checks and alerts

## Architecture

Fiochat is a unified product with two components:

```
fiochat/
├── src/                  # Rust AI service (aichat fork)
│   └── ...               # CLI, REPL, HTTP server, MCP, RAG
├── telegram/             # TypeScript Telegram bridge
│   ├── src/index.ts      # Bot entry point
│   └── ...               # Bot configuration
└── deploy/               # Deployment configs
```

**Components:**
| Component | Language | Purpose |
|-----------|----------|---------|
| AI Service | Rust | LLM integration, function calling, HTTP API |
| Telegram Bot | TypeScript | Telegram bridge, auth, session management |

## Quick Start

### Using Makefile (Easiest)

```bash
# Clone and setup
git clone https://github.com/joon-aca/fiochat.git
cd fiochat
make setup

# Interactive config wizard (recommended)
make config
# Follow the prompts to configure:
# - LLM provider (OpenAI, Claude, Azure, Ollama, etc.)
# - Telegram bot token
# - Allowed user IDs

# Build
make build

# Run (in two terminals)
make dev-ai        # Terminal 1: AI service
make dev-telegram  # Terminal 2: Telegram bot
```

The `make config` wizard will guide you through the setup process. Run `make help` to see all available commands.

## Production Install

For production deployment with systemd services, use the command-based installer:

**Interactive wizard (existing behavior):**

```bash
curl -fsSL https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh | bash
```

The wizard's recommended path is `Install Fio`, which auto-selects:
- Linux with systemd: production install
- macOS: local binary install + optional launchd services

**Unattended / scriptable install (recommended for provisioning):**

```bash
curl -fsSLO https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh
chmod +x install.sh

# Start from the template and fill required values
sudo mkdir -p /etc/fiochat
curl -fsSLo /etc/fiochat/install.env \
  https://raw.githubusercontent.com/joon-aca/fiochat/master/deploy/install.env.example
# edit /etc/fiochat/install.env

# Validate inputs and environment
./install.sh validate --answers /etc/fiochat/install.env --mode production --yes

# Apply installation
./install.sh apply --answers /etc/fiochat/install.env --mode production --yes

# Verify result
./install.sh verify
```

Or as a one-liner with direct flags:

```bash
./install.sh apply --mode production --install-method release --tag v0.2.0 --yes
```

`--tag` is optional (`latest` is the default).

For macOS unattended install:

```bash
./install.sh apply --mode macos --install-method release --yes
```

In `--mode production` (Linux/systemd), the installer will:
- Download and verify the release tarball
- Install to `/opt/fiochat` and `/usr/local/bin/fiochat`
- Create `/usr/local/bin/fio` alias only when it does not conflict with an existing `fio` command
- Create systemd services
- Configure `/etc/fiochat/config.yaml`
- Start services automatically

See [DEPLOYMENT.md](DEPLOYMENT.md) for advanced deployment options.

### Manual Setup

<details>
<summary>Click to expand manual setup steps</summary>

#### 1. Install the AI Service

```bash
# Clone the repository
git clone https://github.com/joon-aca/fiochat.git
cd fiochat

# Build the Rust binary
cargo build --release

# Binary at: target/release/fiochat
```

#### 2. Configure Fiochat

Create `~/.config/fiochat/config.yaml` with both AI service and Telegram configuration:

```bash
mkdir -p ~/.config/fiochat
cp config.fio.example.yaml ~/.config/fiochat/config.yaml
# then edit ~/.config/fiochat/config.yaml
```

Reference template:
- `config.fio.example.yaml` (concise, commented, includes Azure `api_version`)

```yaml
# Telegram Bot Configuration
telegram:
  telegram_bot_token: YOUR_BOT_TOKEN_HERE      # From @BotFather
  allowed_user_ids: "123456789,987654321"      # From @userinfobot
  ops_channel_id: "-1001234567890"             # Optional: for system notifications
  server_name: capraia                         # Your server name
  ai_service_api_url: http://127.0.0.1:8000/v1/chat/completions
  ai_service_model: default
  ai_service_auth_token: Bearer dummy

# AI Service Configuration
model: openai:gpt-4o-mini  # or claude:claude-3-5-sonnet, azure-openai:..., etc.
clients:
- type: openai
  api_key: sk-...

save: true
save_session: null
```

**Note:** Environment variables (e.g., `TELEGRAM_BOT_TOKEN`, `ALLOWED_USER_IDS`) will override config file values.

#### 3. Install the Telegram Bot

```bash
cd telegram
npm install
npm run build
```

#### 4. Start Both Services

```bash
# Terminal 1: AI Service
./target/release/fiochat --serve 127.0.0.1:8000

# Terminal 2: Telegram Bot
cd telegram
npm start
```

</details>

### Test via Telegram

Message your bot: **"Fio, are you online?"**

## Production Deployment

### systemd Services

**AI Service** (`/etc/systemd/system/fiochat.service`):
```ini
[Unit]
Description=Fiochat AI Service
After=network-online.target

[Service]
Type=simple
User=svc
ExecStart=/usr/local/bin/fiochat --serve 127.0.0.1:8000
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

**Telegram Bot** (`/etc/systemd/system/fiochat-telegram.service`):
```ini
[Unit]
Description=Fiochat Telegram Bot
After=network-online.target fiochat.service
Wants=fiochat.service

[Service]
Type=simple
User=svc
WorkingDirectory=/opt/fiochat/telegram
ExecStart=/usr/bin/node dist/index.js
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Enable and Start

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now fiochat.service fiochat-telegram.service
```

## Usage Examples

Once running, chat with your server through Telegram:

- "Fio, show me the active services."
- "What's the status of docker?"
- "Tail the last 50 lines of the nginx error log."
- "Run the backup script."
- "List the cron jobs."

## Telegram Bot Commands

| Command | Description |
|---------|-------------|
| `/start` | Bot introduction |
| `/reset` | Clear conversation context |
| Any text | Relay to AI service |

## CLI Usage

Fiochat also works as a standalone CLI tool (like aichat):

```bash
# Interactive REPL
fio

# Chat-only prompt
fio "explain this error" -f error.log

# Auto mode (default): operational prompt -> plan+confirm
fio "git commit and push the changes"

# Force chat mode
fio --chat "why did this fail?"

# Force plan+confirm mode
fio --plan "restart nginx and tail logs"

# Escape hatch: force execute mode
fio -e "find large files over 100MB"

# Arm/disarm fast execution in current repo scope
fio arm
fio "restart nginx"
fio disarm

# Diagnose command path/collision state
fio doctor
```

`fiochat` remains available as a compatibility alias and defaults to chat mode (`fio --chat` behavior).
`fio arm` is scope-local and time-limited (30 minutes). High-risk commands still require explicit confirmation.

In REPL, both slash and dot commands work:
- `/help` or `.help`
- `/thinking off` or `.thinking off`
- `exit` / `quit` / `:q` to leave

Set default reasoning visibility in config:

```yaml
hide_thinking: true
```

## Upstream

This project is forked from [AIChat](https://github.com/sigoden/aichat). We maintain compatibility with upstream while adding:
- Telegram integration
- DevOps-focused features
- Custom server operation tools

To sync with upstream:
```bash
git fetch upstream
git merge upstream/main
```

## Documentation

- [Telegram Integration](telegram/README.md)
- [MCP Configuration](MCP.md)
- [AIChat Wiki](https://github.com/sigoden/aichat/wiki) (for CLI/REPL features)

## Roadmap

- Health Agent: Automated checks + Telegram alerts
- Cron Agent: Conversational cron editing
- Backup Agent: Snapshot + restore utilities
- Multi-server Dashboard: Fleet-wide monitoring
- TOTP Authentication: For privileged operations

## License

Copyright (c) 2023-2025 aichat-developers, fiochat contributors.

Available under MIT License or Apache License 2.0, at your option.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).
