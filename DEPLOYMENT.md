# Fiochat Deployment Guide

This guide covers deploying both the AI service and Telegram bot components.

## Architecture

Fiochat consists of two components that work together:

1. **AI Service** (Rust): HTTP server providing OpenAI-compatible API
2. **Telegram Bot** (TypeScript): Bridge between Telegram and AI service

## Prerequisites

- Linux server (Ubuntu/Debian recommended)
- Node.js 20+ (for Telegram bot)
- Telegram bot token from [@BotFather](https://t.me/BotFather)
- LLM API key (OpenAI, Claude, Azure, etc.)

## Option 0: Automated Installer (Recommended)

The simplest way to deploy Fiochat is using the automated installer:

```bash
curl -fsSL https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh | bash
```

Or download and inspect first:

```bash
curl -fsSLO https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh
chmod +x install.sh
./install.sh
```

Pin to specific version:

```bash
./install.sh --tag v0.2.0
```

The installer will:
- Guide you through AI provider and Telegram bot configuration
- Download and verify the release tarball
- Install to `/opt/fiochat` (root-owned, read-only)
- Install binary to `/usr/local/bin/fiochat`
- Create system config at `/etc/fiochat/config.yaml` (root:SERVICE_USER, mode 640)
- Create state directory at `/var/lib/fiochat` (owned by service user)
- Create systemd services
- Start services automatically

After installation, your services will be running. Skip to [Verify Deployment](#verify-deployment) below.

## Option 1: systemd Deployment (Manual Build)

### 1. Build and Install AI Service

```bash
# Clone and build
git clone https://github.com/joon-aca/fiochat.git
cd fiochat
cargo build --release

# Install binary
sudo install -m 755 target/release/fiochat /usr/local/bin/fiochat
```

### 2. Configure AI Service

Create `/etc/fiochat/config.yaml` (or `~/.config/fiochat/config.yaml` for development):

```yaml
model: openai:gpt-4o-mini
clients:
- type: openai
  api_key: sk-...

save: true
save_session: null
```

For Azure OpenAI:
```yaml
model: azure-openai:gpt-4o-mini
clients:
- type: azure-openai
  api_base: https://YOUR_RESOURCE.openai.azure.com/
  api_key: YOUR_API_KEY
  models:
  - name: gpt-4o-mini

save: true
save_session: null
```

### 3. Build and Install Telegram Bot

```bash
cd telegram
npm ci
npm run build

# Copy to deployment location
sudo mkdir -p /opt/fiochat/telegram
sudo cp -r dist package.json package-lock.json /opt/fiochat/telegram/
cd /opt/fiochat/telegram
sudo npm ci --omit=dev --no-audit --no-fund
```

### 4. Configure Telegram Bot

**Option A: Use unified config (Recommended)**

Add telegram section to `/etc/fiochat/config.yaml` (or `~/.config/fiochat/config.yaml`):

```yaml
# Telegram Bot Configuration
telegram:
  telegram_bot_token: YOUR_BOT_TOKEN_HERE
  allowed_user_ids: "123456789,987654321"
  server_name: myserver
  ai_service_api_url: http://127.0.0.1:8000/v1/chat/completions
  ai_service_model: default
  ai_service_auth_token: Bearer dummy
```

**Option B: Use environment variables**

Create `/opt/fiochat/telegram/.env`:

```env
TELEGRAM_BOT_TOKEN=your_bot_token_from_botfather
ALLOWED_USER_IDS=123456789,987654321
SERVER_NAME=myserver
AI_SERVICE_API_URL=http://127.0.0.1:8000/v1/chat/completions
AI_SERVICE_MODEL=default
```

**Note:** Environment variables override config file values.

### 5. Create Service User and Directories

```bash
# Create service user
sudo useradd -r -s /bin/false -d /var/lib/fiochat svc

# Create state directory (writable by service)
sudo mkdir -p /var/lib/fiochat
sudo chown -R svc:svc /var/lib/fiochat
sudo chmod 750 /var/lib/fiochat

# Ensure /opt/fiochat is root-owned (read-only for service)
sudo chown -R root:root /opt/fiochat
sudo chmod -R go-w /opt/fiochat

# Set config permissions (readable by service user)
sudo chown root:svc /etc/fiochat/config.yaml
sudo chmod 640 /etc/fiochat/config.yaml
```

### 6. Install systemd Services

```bash
# Copy service files
sudo cp deploy/systemd/fiochat.service /etc/systemd/system/
sudo cp deploy/systemd/fiochat-telegram.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable and start services
sudo systemctl enable --now fiochat.service
sudo systemctl enable --now fiochat-telegram.service
```

### 7. Verify Deployment

<a name="verify-deployment"></a>

```bash
# Check service status
sudo systemctl status fiochat.service
sudo systemctl status fiochat-telegram.service

# View logs
sudo journalctl -u fiochat.service -f
sudo journalctl -u fiochat-telegram.service -f

# Test AI service endpoint
curl -s http://127.0.0.1:8000/v1/models | jq
```

### 8. Test via Telegram

Send a message to your bot: **"Fio, are you online?"**

## Option 2: Docker Deployment

### 1. Create Dockerfiles

**Root Dockerfile** (for AI service):
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/fiochat /usr/local/bin/fiochat
CMD ["fiochat", "--serve", "0.0.0.0:8000"]
```

**telegram/Dockerfile**:
```dockerfile
FROM node:20-slim
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev --no-audit --no-fund
COPY dist ./dist
CMD ["node", "dist/index.js"]
```

### 2. Deploy with Docker Compose

```bash
# Create unified config file
cp config.example.yaml config.yaml
# Edit config.yaml with:
#   - LLM credentials (clients section)
#   - Telegram bot configuration (telegram section)

# Start services
docker-compose up -d

# View logs
docker-compose logs -f
```

**Note:** You can also use environment variables instead of the config file by setting them in the docker-compose.yml environment section.

## Upgrading

### systemd

```bash
cd /path/to/fiochat
git pull

# Rebuild AI service
cargo build --release
sudo install -m 755 target/release/fiochat /usr/local/bin/fiochat
sudo systemctl restart fiochat.service

# Rebuild Telegram bot
cd telegram
npm ci
npm run build
sudo cp -r dist /opt/fiochat/telegram/
sudo systemctl restart fiochat-telegram.service
```

### Docker

```bash
cd /path/to/fiochat
git pull
docker-compose build
docker-compose up -d
```

## Monitoring

### Health Checks

**AI Service**:
```bash
curl http://127.0.0.1:8000/v1/models
```

**Telegram Bot**: Send a test message

### Logs

```bash
# systemd
sudo journalctl -u fiochat.service -u fiochat-telegram.service -f

# Docker
docker-compose logs -f
```

## Security Considerations

1. **API Keys**: Never commit API keys or bot tokens to git
2. **User Authorization**: Always set `ALLOWED_USER_IDS` in Telegram bot config
3. **Network**: AI service should only bind to `127.0.0.1` unless using Docker networking
4. **File Permissions**: Restrict access to `.env` files (`chmod 600`)
5. **Service User**: Run services as unprivileged `svc` user

## Troubleshooting

### AI Service Not Responding

```bash
# Check if running
sudo systemctl status fiochat.service

# Check logs
sudo journalctl -u fiochat.service -n 50

# Test endpoint
curl http://127.0.0.1:8000/v1/models
```

### Telegram Bot Not Responding

```bash
# Check if running
sudo systemctl status fiochat-telegram.service

# Check logs
sudo journalctl -u fiochat-telegram.service -n 50

# Verify bot token
curl "https://api.telegram.org/bot<YOUR_TOKEN>/getMe"
```

### Configuration Issues

```bash
# Verify unified config (recommended approach)
cat ~/.config/fiochat/config.yaml

# Or check environment variables (if using .env file)
cat /opt/fiochat/telegram/.env

# Check config file permissions
ls -la ~/.config/fiochat/config.yaml

# Verify telegram section exists in config
grep -A 5 "^telegram:" ~/.config/fiochat/config.yaml
```

## Multi-Server Deployment

To deploy on multiple servers:

1. Use different `SERVER_NAME` values per server
2. Create unique Telegram bots per server (e.g., `capraia-ops-bot`, `gorgona-ops-bot`)
3. Configure different `TELEGRAM_BOT_TOKEN` values
4. Optionally share the same LLM backend or use separate instances

## Next Steps

- Set up health check monitoring (see `telegram/deploy/scripts/`)
- Configure automatic backups of conversation history
- Set up log rotation
- Enable HTTPS if exposing AI service externally
