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

Add a `telegram` section to `~/.config/fio/config.yaml`:

```yaml
telegram:
  telegram_bot_token: YOUR_BOT_TOKEN_HERE
  allowed_user_ids: "123456789,987654321"
  ops_channel_id: "-1001234567890"  # Optional: for system notifications via fio-notify
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

## System Notifications

The `fio-notify` utility sends system alerts to a Telegram channel for monitoring and ops notifications.

### Setup

1. Create a Telegram channel (New > Channel)
2. Add your bot as administrator to the channel
3. Get the channel ID:
   - Send a message in the channel
   - Stop the bot temporarily
   - Run: `curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates" | grep -oP '"id":-\d+' | head -1`
   - The channel ID will be negative (e.g., `-1003582003509`)
4. Add `ops_channel_id` to your config (see Option 1 above)

### Usage

```bash
# Send a notification to the ops channel
fio-notify "Server maintenance starting"

# Or use environment variable
FIO_NOTIFY_CHANNEL_ID="-1003582003509" fio-notify "Alert message"
```

The `fio-notify` script automatically reads the bot token and channel ID from `~/.config/fio/config.yaml`.

## Production

See the main [fiochat README](../README.md) for full deployment instructions.
