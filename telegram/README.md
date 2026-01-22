# Telegram Integration

This directory contains the Telegram bot bridge component of fiochat.

## Overview

The Telegram integration connects your Telegram bot to the fiochat AI service, enabling chat-based server operations via Telegram.

## Structure

```
telegram/
├── src/
│   └── index.ts          # Main bot entry point
├── deploy/
│   └── scripts/          # Health check and deployment scripts
├── package.json          # Node.js dependencies
├── tsconfig.json         # TypeScript configuration
├── .env.example          # Environment variable template
└── fio-bot.service       # systemd unit file
```

## Configuration

### Option 1: Unified Config (Recommended)

Add a `telegram` section to `~/.config/fiochat/config.yaml`:

```yaml
telegram:
  telegram_bot_token: YOUR_BOT_TOKEN_HERE
  allowed_user_ids: "123456789,987654321"
  server_name: myserver
  ai_service_api_url: http://127.0.0.1:8000/v1/chat/completions
  ai_service_model: default
  ai_service_auth_token: Bearer dummy
```

Run `make config` from the project root for an interactive setup wizard.

### Option 2: Environment Variables

Copy `.env.example` to `.env` and configure:

| Variable | Description |
|----------|-------------|
| `TELEGRAM_BOT_TOKEN` | Bot token from @BotFather |
| `ALLOWED_USER_IDS` | Comma-separated Telegram user IDs |
| `SERVER_NAME` | Name of this server (used in responses) |
| `AI_SERVICE_API_URL` | fiochat service URL (default: `http://127.0.0.1:8000/v1/chat/completions`) |
| `AI_SERVICE_MODEL` | Model name (default: `default`) |
| `AI_SERVICE_AUTH_TOKEN` | Auth token (default: `Bearer dummy`) |

**Note:** Environment variables override config file values.

## Development

```bash
cd telegram
npm install
npm run dev
```

## Bot Commands

- `/start` - Bot introduction
- `/reset` - Clear conversation context
- Any text message - Relay to fiochat AI service

## Production

See the main [fiochat README](../README.md) for full deployment instructions.
