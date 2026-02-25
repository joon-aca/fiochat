# Fiochat Quickstart (Beginner-Friendly)

This guide is for first-time setup on a fresh Linux server.

If you can copy/paste commands, you can do this.

## 0. What You Need

1. A Linux server (Ubuntu/Debian is easiest).
2. A terminal connected to that server.
3. `sudo` access on that server.
4. A Telegram account on your phone.
5. An LLM API key (OpenAI, Claude, Azure, or Ollama).

## 1. Connect To Your Server

From your laptop terminal:

```bash
ssh YOUR_USER@YOUR_SERVER_IP
```

After login, run:

```bash
sudo -v
```

If this works, continue.

## 2. Create Your Telegram Bot (In Telegram App)

1. Open Telegram.
2. Search for `@BotFather`.
3. Open chat with BotFather.
4. Send: `/start`
5. Send: `/newbot`
6. Enter a bot display name (example: `Capraia Ops Bot`).
7. Enter a bot username that ends with `bot` (example: `capraia_ops_bot`).
8. BotFather replies with a token.

Save this token somewhere safe. It looks like:

```text
123456789:AA...long-secret...
```

## 3. Get Your Telegram User ID

1. In Telegram, search for `@userinfobot`.
2. Open chat.
3. Send any message (for example: `hi`).
4. Copy your numeric user ID.

It looks like:

```text
123456789
```

## 4. Create An Alert Channel (Optional, But Recommended)

This lets Fio post alerts to a channel.

1. In Telegram: create a new channel.
2. Add your bot as an admin in that channel.
3. Post one message in the channel (example: `hello`).

Now get the channel ID from your server terminal.

First set your bot token in this command:

```bash
BOT_TOKEN='PASTE_YOUR_BOT_TOKEN_HERE'
curl -s "https://api.telegram.org/bot${BOT_TOKEN}/getUpdates" | grep -oE '"id":-[0-9]+' | head -1
```

You want a **negative** ID, for example:

```text
"id":-1003582003509
```

Save the number part (`-1003582003509`).

If you do not want alert channel messages right now, you can skip this and set it later.

## 5. Download Installer

On the server:

```bash
curl -fsSLO https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh
chmod +x install.sh
```

## 6. Create Your Install Answers File

```bash
sudo mkdir -p /etc/fiochat
sudo curl -fsSLo /etc/fiochat/install.env \
  https://raw.githubusercontent.com/joon-aca/fiochat/master/deploy/install.env.example
```

Open it:

```bash
sudo nano /etc/fiochat/install.env
```

Change at least these values:

1. `FIOCHAT_INSTALL_TAG=v0.2.0` (optional, defaults to latest release)
2. `FIOCHAT_PROVIDER=openai` (or claude / azure-openai / ollama)
3. `FIOCHAT_MODEL=gpt-4o-mini` (or your provider model)
4. `FIOCHAT_OPENAI_API_KEY=...` (or provider-specific key fields)
5. `FIOCHAT_TELEGRAM_BOT_TOKEN=...`
6. `FIOCHAT_ALLOWED_USER_IDS=...` (your numeric Telegram user ID)
7. `FIOCHAT_SERVER_NAME=...` (example: `prod-web-1`)
8. `FIOCHAT_OPS_CHANNEL_ID=...` (optional, for alert channel)

If using Azure OpenAI, also set:

9. `FIOCHAT_AZURE_API_BASE=...`
10. `FIOCHAT_AZURE_OPENAI_API_KEY=...`
11. `FIOCHAT_AZURE_API_VERSION=2025-01-01-preview` (or your required version)

Save and exit nano:

1. Press `Ctrl + O`
2. Press `Enter`
3. Press `Ctrl + X`

## 7. Validate Before Installing

```bash
./install.sh validate --answers /etc/fiochat/install.env --mode production --yes
```

If validation fails, read the missing-field message, fix `/etc/fiochat/install.env`, and run validate again.

## 8. Install

```bash
./install.sh apply --answers /etc/fiochat/install.env --mode production --yes
```

## 9. Verify

```bash
./install.sh verify
```

Check CLI alias/collision status:

```bash
fio doctor || fiochat doctor
```

You can also check services directly:

```bash
sudo systemctl status fiochat.service fiochat-telegram.service --no-pager
```

## 10. First Real Test In Telegram

Open your bot chat and send:

```text
Fio, are you online?
```

If configured correctly, you should get a response.

## 11. Test Alert Channel Posting

If you set `FIOCHAT_OPS_CHANNEL_ID`, send a test message to that channel:

```bash
BOT_TOKEN=$(sudo awk -F= '/^FIOCHAT_TELEGRAM_BOT_TOKEN=/{print $2}' /etc/fiochat/install.env)
CHANNEL_ID=$(sudo awk -F= '/^FIOCHAT_OPS_CHANNEL_ID=/{print $2}' /etc/fiochat/install.env)

curl -s -X POST "https://api.telegram.org/bot${BOT_TOKEN}/sendMessage" \
  -d "chat_id=${CHANNEL_ID}" \
  --data-urlencode "text=✅ Fio alert channel test" >/dev/null && echo "Alert test sent"
```

Check the Telegram channel for the message.

## 12. Useful Commands

View live logs:

```bash
sudo journalctl -u fiochat.service -f
```

```bash
sudo journalctl -u fiochat-telegram.service -f
```

Restart both services:

```bash
sudo systemctl restart fiochat.service fiochat-telegram.service
```

## 13. Safety Notes

1. Treat bot token and API keys like passwords.
2. Never paste secrets into public screenshots or Git commits.
3. Keep `/etc/fiochat/install.env` readable only by admins.

Optional hardening:

```bash
sudo chmod 600 /etc/fiochat/install.env
sudo chown root:root /etc/fiochat/install.env
```

---

If you want the non-technical path later, run the interactive wizard instead:

```bash
curl -fsSL https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh | bash
```
