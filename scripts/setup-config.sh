#!/usr/bin/env bash
# Fiochat interactive setup wizard
set -euo pipefail

# -----------------------------------------------------------------------------
# Colors
# -----------------------------------------------------------------------------
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# -----------------------------------------------------------------------------
# Globals
# -----------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

CONFIG_DIR="${HOME}/.config/fiochat"
CONFIG_FILE="${CONFIG_DIR}/config.yaml"

LEGACY_CONFIG_DIR="${HOME}/.config/aichat"
LEGACY_CONFIG_FILE="${LEGACY_CONFIG_DIR}/config.yaml"

OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"

# Config inspection results (populated by inspect_config)
CFG_EXISTS=""
CFG_MODEL=""
CFG_PROVIDER=""
CFG_APIKEY_STATUS=""   # present | placeholder | missing
CFG_TELEGRAM_STATUS="" # configured | missing
CFG_TG_SERVER_NAME=""
CFG_TG_ALLOWED_IDS=""

# -----------------------------------------------------------------------------
# UI helpers
# -----------------------------------------------------------------------------
hr() { echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; }
title() {
  echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
  echo -e "${BLUE}║   Fiochat Setup Wizard                   ║${NC}"
  echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
}

say() { echo -e "$*"; }
warn() { echo -e "${YELLOW}$*${NC}"; }
ok() { echo -e "${GREEN}$*${NC}"; }
err() { echo -e "${RED}$*${NC}" >&2; }

need_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || {
    err "✗ Missing required command: ${cmd}"
    exit 1
  }
}

# -----------------------------------------------------------------------------
# Prompt helpers
# -----------------------------------------------------------------------------
prompt_input() {
  # prompt_input "Label" "default"
  local prompt="$1"
  local default="${2:-}"
  local value=""

  if [[ -n "$default" ]]; then
    read -r -p "$(echo -e "${GREEN}${prompt}${NC} [${YELLOW}${default}${NC}]: ")" value
    echo "${value:-$default}"
  else
    read -r -p "$(echo -e "${GREEN}${prompt}${NC}: ")" value
    echo "$value"
  fi
}

prompt_yesno() {
  # prompt_yesno "Question" "Y"  -> returns 0 for yes, 1 for no
  local prompt="$1"
  local default="${2:-N}"
  local value=""

  local suffix="(y/N)"
  if [[ "$default" == "Y" ]]; then suffix="(Y/n)"; fi

  read -r -p "$(echo -e "${GREEN}${prompt}${NC} ${suffix}: ")" value
  value="${value:-$default}"

  if [[ "$value" == "y" || "$value" == "Y" ]]; then
    return 0
  fi
  return 1
}

prompt_choice() {
  # prompt_choice "Label" "default" -> echoes chosen value
  local prompt="$1"
  local default="$2"
  local value=""
  read -r -p "$(echo -e "${GREEN}${prompt}${NC} [${YELLOW}${default}${NC}]: ")" value
  echo "${value:-$default}"
}

validated_choice() {
  # validated_choice "Label" "default" "1 2 3 4" -> validates and echoes chosen value
  # Returns 1 if user cancels with 'q'
  local prompt="$1"
  local default="$2"
  local valid_options="$3"
  local show_help="${4:-}"
  
  while true; do
    local value=""
    read -r -p "$(echo -e "${GREEN}${prompt}${NC} [${YELLOW}${default}${NC}]: ")" value
    value="${value:-$default}"
    
    case "$value" in
      \?|help)
        if [[ -n "$show_help" ]]; then
          echo ""
          return 2  # Signal to reprint menu
        fi
        ;;
      q|quit)
        echo ""
        warn "⏭ Cancelled."
        return 1
        ;;
      *)
        # Check if value is in valid_options
        if [[ " $valid_options " =~ " $value " ]]; then
          echo "$value"
          return 0
        else
          echo ""
          err "Invalid choice: '$value' — please enter one of: $valid_options"
          echo ""
        fi
        ;;
    esac
  done
}

prompt_secret() {
  # prompt_secret "Label" -> echoes secret value
  local label="$1"
  local value=""

  # We want:
  # - hidden input
  # - works on macOS + Linux
  # - explicit reassurance + confirmation
  #
  # Use stdin if it's a TTY; otherwise try /dev/tty; otherwise fail.
  echo -e "${GREEN}${label}${NC} ${YELLOW}(input hidden; paste is ok)${NC}" >&2
  echo -n "> " >&2

  if [[ -t 0 ]]; then
    # stdin is a tty
    IFS= read -r -s value
    echo "" >&2
  elif [[ -r /dev/tty ]]; then
    # fallback to controlling terminal
    stty -echo </dev/tty
    IFS= read -r value </dev/tty || true
    stty echo </dev/tty
    echo "" >&2
  else
    echo "" >&2
    err "✗ Cannot read hidden input (no TTY available)."
    err "  Tip: run interactively, or set credentials via environment variables."
    return 1
  fi

  if [[ -n "$value" ]]; then
    echo -e "${GREEN}✓ captured${NC} (${#value} chars)" >&2
  else
    echo -e "${YELLOW}⚠ captured empty value${NC}" >&2
  fi

  echo "$value"
}

# -----------------------------------------------------------------------------
# YAML helpers (simple, not a full parser)
# -----------------------------------------------------------------------------
backup_config() {
  if [[ -f "$CONFIG_FILE" ]]; then
    local ts
    ts="$(date +%Y%m%d-%H%M%S)"
    local bak="${CONFIG_FILE}.bak-${ts}"
    cp "$CONFIG_FILE" "$bak"
    ok "✓ Backup created: ${bak}"
  fi
}

remove_telegram_section_inplace() {
  # Remove telegram: section (top-level) from config.
  # Assumes telegram is a top-level key.
  local tmpfile
  tmpfile="$(mktemp)"
  awk '
    BEGIN {in_tg=0}
    /^[^[:space:]]/ { if (in_tg==1) in_tg=0 }
    /^[[:space:]]*telegram:[[:space:]]*$/ { in_tg=1; next }
    { if (in_tg==0) print }
  ' "$CONFIG_FILE" > "$tmpfile"
  mv "$tmpfile" "$CONFIG_FILE"
}

extract_telegram_section() {
  # Prints telegram section (from telegram: to end of its block) if present.
  if [[ ! -f "$CONFIG_FILE" ]]; then return 0; fi
  awk '
    BEGIN {in_tg=0}
    /^[[:space:]]*telegram:[[:space:]]*$/ {in_tg=1}
    /^[^[:space:]]/ { if (in_tg==1 && $0 !~ /^telegram:/) exit }
    { if (in_tg==1) print }
  ' "$CONFIG_FILE" || true
}

install_telegram_section() {
  # Appends telegram section with a nice header
  local bot_token="$1"
  local user_ids="$2"
  local server_name="$3"
  local ai_service_url="$4"
  local ai_service_model="$5"
  local ai_service_token="$6"
  local ops_channel_id="${7:-}"

  cat >> "$CONFIG_FILE" <<EOF

# ==============================================================================
# Telegram Bot Configuration
# ==============================================================================
# Bot token: get from @BotFather
# User ID(s): get from @userinfobot
# Ops channel ID: for system notifications (optional, negative number for channels)
telegram:
  telegram_bot_token: ${bot_token}
  allowed_user_ids: "${user_ids}"
  server_name: ${server_name}
  ai_service_api_url: ${ai_service_url}
  ai_service_model: ${ai_service_model}
  ai_service_auth_token: ${ai_service_token}
  ai_service_session_namespace: ${server_name}
EOF

  # Add ops_channel_id if provided
  if [[ -n "$ops_channel_id" ]]; then
    echo "  ops_channel_id: \"${ops_channel_id}\"" >> "$CONFIG_FILE"
  fi
}

# -----------------------------------------------------------------------------
# Config inspection
# -----------------------------------------------------------------------------
check_port_in_use() {
  # check_port_in_use PORT
  local port="$1"
  local out
  if [[ -z "$port" ]]; then
    return 1
  fi

  if command -v lsof >/dev/null 2>&1; then
    out=$(lsof -nP -iTCP:"${port}" -sTCP:LISTEN 2>/dev/null || true)
    if [[ -n "$out" ]]; then
      echo "$out"
      return 0
    else
      return 1
    fi
  elif command -v ss >/dev/null 2>&1; then
    out=$(ss -ltnp "sport = :${port}" 2>/dev/null || true)
    if [[ -n "$out" && ! "$out" =~ "State" ]]; then
      echo "$out"
      return 0
    else
      return 1
    fi
  elif command -v netstat >/dev/null 2>&1; then
    out=$(netstat -ltnp 2>/dev/null | grep ":${port} " || true)
    if [[ -n "$out" ]]; then
      echo "$out"
      return 0
    else
      return 1
    fi
  else
    # Fallback: try to connect (true means something is listening)
    if python3 - <<PY >/dev/null 2>&1
import socket,sys
try:
 s=socket.socket(); s.settimeout(0.5); s.connect(('127.0.0.1', int(sys.argv[1]))); s.close(); print('open')
except Exception:
 pass
PY
    then
      echo "port ${port} seems open (connection succeeded)"
      return 0
    else
      return 1
    fi
  fi
}

inspect_config() {
  CFG_EXISTS=""
  CFG_MODEL=""
  CFG_PROVIDER=""
  CFG_APIKEY_STATUS="missing"
  CFG_TELEGRAM_STATUS="missing"
  CFG_TG_SERVER_NAME=""
  CFG_TG_ALLOWED_IDS=""

  if [[ ! -f "$CONFIG_FILE" ]]; then
    return 0
  fi

  CFG_EXISTS="1"

  local model_line
  model_line="$(grep -E '^model:[[:space:]]*' "$CONFIG_FILE" 2>/dev/null || true)"
  if [[ -n "$model_line" ]]; then
    CFG_MODEL="$(echo "$model_line" | sed -E 's/^model:[[:space:]]*//')"
  fi

  # Provider inference from clients type
  if grep -qE '^[[:space:]]*-[[:space:]]*type:[[:space:]]*openai' "$CONFIG_FILE"; then
    CFG_PROVIDER="OpenAI"
  elif grep -qE '^[[:space:]]*-[[:space:]]*type:[[:space:]]*claude' "$CONFIG_FILE"; then
    CFG_PROVIDER="Anthropic Claude"
  elif grep -qE '^[[:space:]]*-[[:space:]]*type:[[:space:]]*azure-openai' "$CONFIG_FILE"; then
    CFG_PROVIDER="Azure OpenAI"
  elif grep -qE '^[[:space:]]*-[[:space:]]*type:[[:space:]]*ollama' "$CONFIG_FILE"; then
    CFG_PROVIDER="Ollama"
  else
    CFG_PROVIDER="unknown"
  fi

  # API key: present vs placeholder vs missing
  if grep -qE '^[[:space:]]*api_key:[[:space:]]*' "$CONFIG_FILE"; then
    if grep -qE '^[[:space:]]*api_key:[[:space:]]*(YOUR_API_KEY_HERE|YOUR_API_KEY|sk-REPLACE|REPLACE_ME)' "$CONFIG_FILE"; then
      CFG_APIKEY_STATUS="placeholder"
    else
      # It exists and doesn't match the common placeholders
      CFG_APIKEY_STATUS="present"
    fi
  else
    CFG_APIKEY_STATUS="missing"
  fi

  # Telegram section
  if grep -qE '^[[:space:]]*telegram:[[:space:]]*$' "$CONFIG_FILE"; then
    CFG_TELEGRAM_STATUS="configured"
    CFG_TG_SERVER_NAME="$(awk '/^[[:space:]]*telegram:[[:space:]]*$/{in=1;next} in && /^[^[:space:]]/{exit} in && /^[[:space:]]*server_name:/{print $2; exit}' "$CONFIG_FILE" 2>/dev/null || true)"
    CFG_TG_ALLOWED_IDS="$(awk '/^[[:space:]]*telegram:[[:space:]]*$/{in=1;next} in && /^[^[:space:]]/{exit} in && /^[[:space:]]*allowed_user_ids:/{sub(/^[^"]*"/,""); sub(/".*/,""); print; exit}' "$CONFIG_FILE" 2>/dev/null || true)"
  fi
}

print_config_summary() {
  inspect_config

  if [[ -z "$CFG_EXISTS" ]]; then
    warn "No config found yet:"
    say "  - ${CONFIG_FILE}"
    return 0
  fi

  warn "Existing config detected: ${CONFIG_FILE}"
  echo ""
  say "${BLUE}Config summary:${NC}"
  if [[ -n "$CFG_MODEL" ]]; then
    say "  - Model: ${CFG_MODEL}"
  else
    say "  - Model: ${RED}not set${NC}"
  fi
  say "  - Provider: ${CFG_PROVIDER}"
  case "$CFG_APIKEY_STATUS" in
    present) say "  - API key: ${GREEN}present${NC}" ;;
    placeholder) say "  - API key: ${RED}placeholder (needs fixing)${NC}" ;;
    missing) say "  - API key: ${RED}missing${NC}" ;;
  esac
  if [[ "$CFG_TELEGRAM_STATUS" == "configured" ]]; then
    say "  - Telegram bot: ${GREEN}configured${NC}"
    [[ -n "$CFG_TG_SERVER_NAME" ]] && say "    • server_name: ${CFG_TG_SERVER_NAME}"
    [[ -n "$CFG_TG_ALLOWED_IDS" ]] && say "    • allowed_user_ids: ${CFG_TG_ALLOWED_IDS}"
  else
    say "  - Telegram bot: ${YELLOW}not configured${NC}"
  fi

  echo ""
  # Overall health hint
  local ai_ok="0"
  if [[ -n "$CFG_MODEL" && "$CFG_APIKEY_STATUS" == "present" ]]; then ai_ok="1"; fi

  if [[ "$ai_ok" == "1" && "$CFG_TELEGRAM_STATUS" == "configured" ]]; then
    ok "Status: ✓ AI looks usable; ✓ Telegram looks usable"
  elif [[ "$ai_ok" == "1" ]]; then
    warn "Status: ✓ AI looks usable; ⚠ Telegram missing"
  else
    warn "Status: ⚠ AI config is incomplete; Telegram may also be missing"
  fi
}

# -----------------------------------------------------------------------------
# Provider setup (AI)
# -----------------------------------------------------------------------------
select_provider() {
  while true; do
    # NOTE: This function is called via command substitution: provider="$(select_provider)"
    # Bash captures STDOUT in command substitution, so we must print UI to STDERR.
    echo -e "${BLUE}AI provider setup${NC}" >&2
    echo "Pick who will answer Fio's requests:" >&2
    echo "  1) OpenAI        - easiest default if you have an OpenAI key" >&2
    echo "  2) Claude        - requires Anthropic API key" >&2
    echo "  3) Azure OpenAI  - requires Azure resource URL + deployment name" >&2
    echo "  4) Ollama (local)- requires ollama running locally" >&2
    echo "  5) Manual        - create template only (you edit YAML)" >&2
    echo "" >&2
    echo -e "${YELLOW}Tip:${NC} press Enter for [1]. Type ${YELLOW}?${NC} to reprint help. Type ${YELLOW}q${NC} to cancel." >&2
    echo "" >&2

    local choice
    read -r -p "$(echo -e "${GREEN}Provider${NC} [${YELLOW}1${NC}]: ")" choice
    choice="${choice:-1}"

    case "$choice" in
      1) echo "openai"; return 0 ;;
      2) echo "claude"; return 0 ;;
      3) echo "azure-openai"; return 0 ;;
      4) echo "ollama"; return 0 ;;
      5) echo "manual"; return 0 ;;
      \?|help)
        echo "" >&2
        # loop will reprint menu
        ;;
      q|quit)
        echo "" >&2
        warn "⏭ Cancelled provider selection." >&2
        return 1
        ;;
      *)
        echo "" >&2
        err "Invalid choice: '$choice' — please enter 1, 2, 3, 4, or 5." >&2
        echo "" >&2
        ;;
    esac
  done
}

write_ai_config() {
  # write_ai_config provider keep_telegram(true/false)
  local provider="$1"
  local keep_telegram="${2:-true}"

  mkdir -p "$CONFIG_DIR"

  local telegram_block=""
  if [[ "$keep_telegram" == "true" ]]; then
    telegram_block="$(extract_telegram_section || true)"
  fi

  # Remove old telegram before rewriting (we'll re-append if requested)
  if [[ -f "$CONFIG_FILE" ]]; then
    remove_telegram_section_inplace || true
  fi

  case "$provider" in
    openai)
      echo ""
      echo -e "${BLUE}OpenAI configuration${NC}"
      echo "You'll provide an OpenAI key and choose a model."
      echo ""
      local api_key model
      api_key="$(prompt_secret "OpenAI key value")"
      model="$(prompt_input "Model name" "gpt-4o-mini")"

      cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.

model: openai:${model}
clients:
- type: openai
  api_key: ${api_key}

save: true
save_session: null
EOF
      ;;

    claude)
      echo ""
      echo -e "${BLUE}Anthropic Claude configuration${NC}"
      echo "You'll provide an Anthropic key and choose a model."
      echo ""
      local api_key model
      api_key="$(prompt_secret "Anthropic key value")"
      model="$(prompt_input "Model name" "claude-3-5-sonnet-20241022")"

      cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.

model: claude:${model}
clients:
- type: claude
  api_key: ${api_key}

save: true
save_session: null
EOF
      ;;

    azure-openai)
      echo ""
      echo -e "${BLUE}Azure OpenAI configuration${NC}"
      echo "You'll provide your Azure resource URL, key, and deployment name."
      echo ""
      local api_base api_key model
      api_base="$(prompt_input "Azure API base URL" "https://YOUR_RESOURCE.openai.azure.com/")"
      api_key="$(prompt_secret "Azure key value")"
      model="$(prompt_input "Deployment name" "gpt-4o-mini")"

      cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.

model: azure-openai:${model}
clients:
- type: azure-openai
  api_base: ${api_base}
  api_key: ${api_key}
  models:
  - name: ${model}

save: true
save_session: null
EOF
      ;;

    ollama)
      echo ""
      echo -e "${BLUE}Ollama (local) configuration${NC}"
      echo "No external key required. Ollama must be running."
      echo ""
      local api_base model
      api_base="$(prompt_input "Ollama API base URL" "http://localhost:11434")"
      model="$(prompt_input "Model name" "llama3.2")"

      cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.

model: ollama:${model}
clients:
- type: ollama
  api_base: ${api_base}

save: true
save_session: null
EOF
      ;;

    manual)
      echo ""
      warn "Manual configuration selected."
      echo "I'll write a template config. You'll edit it afterward."
      echo ""
      cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.
# Edit this file to configure your LLM provider.
# See: https://github.com/sigoden/aichat/wiki/Configuration-Guide

model: openai:gpt-4o-mini
clients:
- type: openai
  api_key: YOUR_API_KEY_HERE

save: true
save_session: null
EOF
      ;;
  esac

  # Re-append telegram section if requested and present
  if [[ "$keep_telegram" == "true" && -n "$telegram_block" ]]; then
    echo "" >> "$CONFIG_FILE"
    echo "$telegram_block" >> "$CONFIG_FILE"
  else
    echo "" >> "$CONFIG_FILE"
    echo "# Telegram bot configuration can be added below." >> "$CONFIG_FILE"
  fi

  ok "✓ AI config written: ${CONFIG_FILE}"
}

# -----------------------------------------------------------------------------
# Telegram setup
# -----------------------------------------------------------------------------
configure_telegram() {
  # configure_telegram mode(update|add) existing_defaults_allowed_ids existing_defaults_server_name
  local mode="${1:-add}"

  echo ""
  echo -e "${BLUE}Telegram setup${NC}"
  echo "This config controls:"
  echo "  - which Telegram bot token to use"
  echo "  - which Telegram user IDs are allowed"
  echo "  - the server name shown in responses"
  echo ""

  if prompt_yesno "Store Telegram settings in config.yaml (recommended)?" "Y"; then
    echo ""
  else
    warn "Ok — you can use telegram/.env instead. I can still write unified config for you now, but env vars will override it."
    echo ""
  fi

  echo -e "${BLUE}You will need:${NC}"
  echo "  - Bot token from @BotFather"
  echo "  - Your user ID from @userinfobot"
  echo ""

  local bot_token user_ids default_ids server_name default_server
  default_ids="${CFG_TG_ALLOWED_IDS:-}"
  default_server="${CFG_TG_SERVER_NAME:-$(hostname -s 2>/dev/null || echo "server")}"

  bot_token="$(prompt_secret "Telegram bot token")"
  user_ids="$(prompt_input "Allowed Telegram user IDs (comma-separated)" "${default_ids}")"
  server_name="$(prompt_input "Server name" "${default_server}")"

  echo ""
  echo -e "${BLUE}Ops notifications channel (optional)${NC}"
  echo "For system alerts/notifications to a Telegram channel."
  echo ""
  echo -e "${YELLOW}How to get your channel ID:${NC}"
  echo "  1. Create a channel in Telegram (New > Channel)"
  echo "  2. Add your bot as administrator to the channel"
  echo "  3. Send a message in the channel"
  echo "  4. Stop the bot temporarily and run:"
  echo "     curl -s \"https://api.telegram.org/bot<TOKEN>/getUpdates\" | grep -oP '\"id\":-\\d+' | head -1"
  echo "  5. The channel ID will be negative (e.g., -1003582003509)"
  echo ""
  echo -e "${YELLOW}Tip:${NC} Press Enter to skip if you don't need channel notifications."
  echo ""
  local ops_channel_id
  ops_channel_id="$(prompt_input "Ops channel ID (optional)" "")"

  echo ""
  echo -e "${BLUE}Connection settings (usually keep defaults)${NC}"
  echo "Default assumes:"
  echo "  - AI service runs on the same machine"
  echo "  - listens on 127.0.0.1:8000"
  echo "  - no auth required between bot and service"
  echo ""
  local ai_service_url ai_service_model ai_service_token
  if prompt_yesno "Change connection settings anyway?" "N"; then
    echo ""
    echo -e "${BLUE}AI service URL${NC}"
    echo "Where the bot sends requests."
    ai_service_url="$(prompt_input "URL" "http://127.0.0.1:8000/v1/chat/completions")"

    echo ""
    echo -e "${BLUE}AI service model${NC}"
    echo "Usually 'default'. Only change if your AI service expects a specific model name."
    ai_service_model="$(prompt_input "Model" "default")"

    echo ""
    echo -e "${BLUE}AI service auth token${NC}"
    echo "Used only if your AI service requires auth. Press Enter to disable."
    ai_service_token="$(prompt_secret "Auth token")"
    ai_service_token="${ai_service_token:-Bearer <no-auth>}"
  else
    ai_service_url="http://127.0.0.1:8000/v1/chat/completions"
    ai_service_model="default"
    ai_service_token="Bearer <no-auth>"
  fi

  # Write/update telegram section
  backup_config
  if [[ -f "$CONFIG_FILE" ]] && grep -qE '^[[:space:]]*telegram:[[:space:]]*$' "$CONFIG_FILE"; then
    remove_telegram_section_inplace
  fi

  install_telegram_section "$bot_token" "$user_ids" "$server_name" "$ai_service_url" "$ai_service_model" "$ai_service_token" "$ops_channel_id"

  echo ""
  if [[ "$mode" == "update" ]]; then
    ok "✓ Telegram config updated in: ${CONFIG_FILE}"
  else
    ok "✓ Telegram config added to: ${CONFIG_FILE}"
  fi

  echo ""
  echo -e "${BLUE}Telegram configuration recap${NC}"
  echo "  - server_name: ${server_name}"
  echo "  - allowed_user_ids: ${user_ids}"
  echo "  - ai_service_api_url: ${ai_service_url}"
  if [[ -n "$ops_channel_id" ]]; then
    echo "  - ops_channel_id: ${ops_channel_id}"
  fi
}

# -----------------------------------------------------------------------------
# Dev journey
# -----------------------------------------------------------------------------
dev_journey() {
  hr
  echo -e "${BLUE}Development setup${NC}"
  hr
  echo ""

  print_config_summary

  if [[ -z "$CFG_EXISTS" ]]; then
    echo ""
    warn "No config exists yet. We'll create one now."
    echo ""
    local provider
    provider="$(select_provider)" || {
      warn "⏭ Setup cancelled. No changes made."
      return 0
    }
    backup_config
    write_ai_config "$provider" "false"
    inspect_config
    echo ""
    configure_telegram "add"
  else
    echo ""
    echo -e "${BLUE}What would you like to do?${NC}"
    echo "  1) Keep as-is (recommended if status looks good)"
    echo "  2) Update AI provider settings"
    echo "  3) Update Telegram settings"
    echo "  4) Reset everything (rebuild AI + Telegram)"
    echo ""
    echo -e "${YELLOW}Tip:${NC} Type ${YELLOW}q${NC} to cancel."
    echo ""
    local action
    action="$(validated_choice "Choose" "1" "1 2 3 4")" || return 0

    case "$action" in
      1)
        ok "✓ Keeping current configuration."
        ;;
      2)
        echo ""
        warn "We will rebuild the AI provider section."
        echo "Telegram settings will be preserved if present."
        echo ""
        local provider
        provider="$(select_provider)" || {
          warn "⏭ Cancelled. No changes made."
          return 0
        }
        backup_config
        write_ai_config "$provider" "true"
        ;;
      3)
        inspect_config
        configure_telegram "update"
        ;;
      4)
        echo ""
        warn "This will rebuild AI + Telegram configuration."
        echo "A backup will be created first."
        echo ""
        local provider
        provider="$(select_provider)" || {
          warn "⏭ Cancelled. No changes made."
          return 0
        }
        backup_config
        write_ai_config "$provider" "false"
        inspect_config
        configure_telegram "add"
        ;;
    esac
  fi

  echo ""
  hr
  ok "✓ Setup complete"
  hr
  echo ""
  echo -e "${YELLOW}Config file:${NC}"
  echo "  ${CONFIG_FILE}"
  echo ""

  # Check if release binaries exist and offer to start them
  local fio_release="${PROJECT_ROOT}/target/release/fiochat"
  local telegram_built="${PROJECT_ROOT}/telegram/dist/index.js"

  if [[ -f "$fio_release" && -f "$telegram_built" ]]; then
    echo -e "${BLUE}Release binaries detected${NC}"
    echo "I can start the services now using your built release binaries."
    echo ""
    if prompt_yesno "Start services now?" "Y"; then
      echo ""
      echo -e "${BLUE}Starting AI service...${NC}"
      echo "Endpoint: http://127.0.0.1:8000"
      nohup "$fio_release" --serve 127.0.0.1:8000 > /tmp/fiochat-ai.log 2>&1 &
      local ai_pid=$!
      echo "  PID: $ai_pid (logs: /tmp/fiochat-ai.log)"

      sleep 2

      echo ""
      echo -e "${BLUE}Starting Telegram bot...${NC}"
      cd "${PROJECT_ROOT}/telegram" && nohup node dist/index.js > /tmp/fiochat-telegram.log 2>&1 &
      local tg_pid=$!
      echo "  PID: $tg_pid (logs: /tmp/fiochat-telegram.log)"

      echo ""
      ok "✓ Services started in background"
      echo ""
      echo -e "${YELLOW}Monitor:${NC}"
      echo "  tail -f /tmp/fiochat-ai.log"
      echo "  tail -f /tmp/fiochat-telegram.log"
      echo ""
      echo -e "${YELLOW}Stop:${NC}"
      echo "  kill $ai_pid $tg_pid"
      echo ""
      echo -e "${YELLOW}Test:${NC}"
      echo "  Message your bot \"ping\""
      echo ""
    else
      echo ""
      echo -e "${YELLOW}Next steps (dev):${NC}"
      echo -e "  Run AI:       ${GREEN}${fio_release} --serve 127.0.0.1:8000${NC}"
      echo -e "  Run Telegram: ${GREEN}cd telegram && node dist/index.js${NC}"
      echo "  Or use:       make dev-ai  &&  make dev-telegram"
      echo ""
    fi
  else
    echo -e "${YELLOW}Next steps (dev):${NC}"
    echo -e "  1) Build:        ${GREEN}make build${NC}"
    echo -e "  2) Run AI:       ${GREEN}make dev-ai${NC}"
    echo -e "  3) Run Telegram: ${GREEN}make dev-telegram${NC}"
    echo "  4) Test:         message your bot \"ping\""
    echo ""
  fi
}

# -----------------------------------------------------------------------------
# Inspect-only journey
# -----------------------------------------------------------------------------
inspect_only() {
  hr
  echo -e "${BLUE}Inspect / verify configuration${NC}"
  hr
  echo ""

  print_config_summary
  echo ""

  if [[ -z "$CFG_EXISTS" ]]; then
    warn "No config found. Create one with:"
    echo "  ${GREEN}make config${NC}"
    echo ""
    return 0
  fi

  # Actionable guidance based on status
  if [[ -z "$CFG_MODEL" || "$CFG_APIKEY_STATUS" != "present" ]]; then
    warn "AI config needs attention."
    echo "Recommended:"
    echo "  ${GREEN}make config${NC}  (choose 'Update AI provider settings')"
    echo ""
  else
    ok "AI config looks usable."
    echo ""
  fi

  if [[ "$CFG_TELEGRAM_STATUS" != "configured" ]]; then
    warn "Telegram config is missing."
    echo "Recommended:"
    echo "  ${GREEN}make config${NC}  (choose 'Update Telegram settings')"
    echo ""
  else
    ok "Telegram config looks usable."
    echo ""
  fi
}

# -----------------------------------------------------------------------------
# Release installer helpers (Linux)
# -----------------------------------------------------------------------------
detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) echo "amd64" ;;
    aarch64|arm64) echo "arm64" ;;
    *)
      err "✗ Unsupported architecture: ${arch}"
      exit 1
      ;;
  esac
}

download_file() {
  local url="$1"
  local out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$out" "$url"
  else
    err "✗ Need curl or wget to download releases"
    exit 1
  fi
}

verify_sha256() {
  local sha_file="$1"
  local tar_file="$2"
  need_cmd sha256sum
  (cd "$(dirname "$tar_file")" && sha256sum -c "$(basename "$sha_file")")
}

install_release_tarball() {
  local owner_repo="$1"   # e.g. joon-aca/fiochat
  local version="$2"      # e.g. v0.2.0
  local arch="$3"         # amd64|arm64

  local tmpdir
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' RETURN

  local base="fiochat-${version}-linux-${arch}.tar.gz"
  local tar_url="https://github.com/${owner_repo}/releases/download/${version}/${base}"
  local sha_url="${tar_url}.sha256"

  echo -e "${BLUE}Release install${NC}"
  echo "  - Download: ${tar_url}"
  echo "  - Verify:   ${sha_url}"
  echo ""

  download_file "$tar_url" "${tmpdir}/${base}"
  download_file "$sha_url" "${tmpdir}/${base}.sha256"

  echo -e "${BLUE}Verifying checksum...${NC}"
  (cd "$tmpdir" && verify_sha256 "${base}.sha256" "${base}")
  ok "✓ Checksum OK"

  echo -e "${BLUE}Extracting...${NC}"
  tar -xzf "${tmpdir}/${base}" -C "$tmpdir"

  local extracted_dir
  extracted_dir="$(find "$tmpdir" -maxdepth 1 -type d -name "fiochat-*" | head -n 1)"
  if [[ -z "$extracted_dir" ]]; then
    err "✗ Could not find extracted fiochat directory in tarball"
    exit 1
  fi

  echo -e "${BLUE}Installing to /opt/fiochat...${NC}"
  sudo rm -rf /opt/fiochat
  sudo mkdir -p /opt/fiochat
  sudo cp -a "${extracted_dir}/." /opt/fiochat/

  if [[ -f "/opt/fiochat/bin/fiochat" ]]; then
    echo -e "${BLUE}Installing fiochat to /usr/local/bin/fiochat...${NC}"
    sudo install -m 755 /opt/fiochat/bin/fiochat /usr/local/bin/fiochat
  elif [[ -f "/opt/fiochat/fiochat" ]]; then
    echo -e "${BLUE}Installing fiochat to /usr/local/bin/fiochat...${NC}"
    sudo install -m 755 /opt/fiochat/fiochat /usr/local/bin/fiochat
  else
    err "✗ Release tarball missing fiochat binary (expected bin/fiochat or fiochat)"
    exit 1
  fi

  sudo chown -R root:root /opt/fiochat
  sudo chmod -R go-w /opt/fiochat
  ok "✓ Installed release to /opt/fiochat"
}

# -----------------------------------------------------------------------------
# Production journey (Linux)
# -----------------------------------------------------------------------------
production_journey() {
  hr
  echo -e "${BLUE}Production install (systemd)${NC}"
  hr
  echo ""

  if [[ "$OS_NAME" != "linux" ]]; then
    err "✗ Production/systemd install is intended for Linux."
    err "  Use Development setup on macOS."
    exit 1
  fi

  if [[ ! -d "/etc/systemd/system" ]]; then
    err "✗ systemd not detected (/etc/systemd/system missing)."
    exit 1
  fi

  if ! command -v sudo >/dev/null 2>&1; then
    err "✗ sudo not found; cannot perform system install."
    exit 1
  fi

  local systemd_dir="${PROJECT_ROOT}/deploy/systemd"
  if [[ ! -d "$systemd_dir" ]]; then
    err "✗ systemd unit files not found at: ${systemd_dir}"
    err "  Run this wizard from within the fiochat repo checkout."
    exit 1
  fi

  echo -e "${BLUE}Plan${NC}"
  echo "I will:"
  echo "  - Install fiochat to /opt/fiochat (root-owned)"
  echo "  - Install config to /etc/fiochat/config.yaml"
  echo "  - Create state dir /var/lib/fiochat"
  echo "  - Install systemd services: fiochat.service, fiochat-telegram.service"
  echo ""

  if ! prompt_yesno "Proceed with production install?" "Y"; then
    warn "⏭ Skipping production install."
    return 0
  fi

  echo ""
  echo -e "${BLUE}Service user${NC}"
  echo "Recommended: dedicated user 'svc' (safer; least privilege)"
  echo "Alternative: current user (convenient; fine for small VPS)"
  echo ""
  local current_user service_user use_current_user
  current_user="$(id -un)"
  echo "  1) ${current_user} (current user)"
  echo "  2) svc (dedicated service user; auto-create)"
  echo ""
  echo -e "${YELLOW}Tip:${NC} Type ${YELLOW}q${NC} to cancel."
  echo ""
  local user_choice
  user_choice="$(validated_choice "Choose" "2" "1 2")" || return 0

  use_current_user=""
  if [[ "$user_choice" == "1" ]]; then
    service_user="${current_user}"
    use_current_user="1"
  else
    service_user="svc"
    if ! id svc >/dev/null 2>&1; then
      echo ""
      echo -e "${BLUE}Creating service user 'svc'...${NC}"
      sudo useradd -r -s /bin/false -d /var/lib/fiochat svc
      ok "✓ Created user svc"
    fi
  fi

  echo ""
  echo -e "${BLUE}Install method${NC}"
  echo "  1) GitHub Release (recommended) - fastest and predictable"
  echo "  2) Build locally / manual deploy - you will build and copy to /opt"
  echo ""
  echo -e "${YELLOW}Tip:${NC} Type ${YELLOW}q${NC} to cancel."
  echo ""
  local install_method
  install_method="$(validated_choice "Choose" "1" "1 2")" || return 0

  local release_installed=""
  if [[ "$install_method" == "1" ]]; then
    local default_version arch owner_repo release_version
    default_version="${FIOCHAT_INSTALL_TAG:-v0.0.0}"
    release_version="$(prompt_input "Release tag (e.g. v0.2.0)" "${default_version}")"
    arch="$(detect_arch)"
    owner_repo="joon-aca/fiochat"

    need_cmd tar
    need_cmd sha256sum

    install_release_tarball "${owner_repo}" "${release_version}" "${arch}"
    release_installed="1"
  else
    warn "Ok — you will build and deploy manually."
  fi

  echo ""
  echo -e "${BLUE}Configuration${NC}"
  echo "We need a valid config to install to /etc/fiochat/config.yaml."
  echo ""
  echo "  1) Use existing user config (${CONFIG_FILE})"
  echo "  2) Create/repair config now (recommended)"
  echo "  3) Install template only (you will edit /etc/fiochat/config.yaml later)"
  echo ""
  echo -e "${YELLOW}Tip:${NC} Type ${YELLOW}q${NC} to cancel."
  echo ""
  local cfg_source
  cfg_source="$(validated_choice "Choose" "2" "1 2 3")" || return 0

  case "$cfg_source" in
    1)
      print_config_summary
      if [[ -z "$CFG_EXISTS" ]]; then
        err "✗ No user config exists to use."
        cfg_source="2"
      fi
      ;;
    2)
      echo ""
      print_config_summary
      echo ""
      warn "We will (re)build your AI config now."
      local provider
      provider="$(select_provider)" || {
        warn "⏭ Installation cancelled."
        exit 0
      }
      backup_config
      write_ai_config "$provider" "true"

      inspect_config
      if [[ "$CFG_TELEGRAM_STATUS" != "configured" ]]; then
        echo ""
        warn "Telegram is not configured yet. We'll configure it now."
        configure_telegram "add"
      else
        if prompt_yesno "Update Telegram settings now?" "N"; then
          configure_telegram "update"
        else
          ok "✓ Keeping existing Telegram settings."
        fi
      fi
      ;;
    3)
      echo ""
      warn "Installing a template to /etc. Services may fail until you edit it."
      mkdir -p "$CONFIG_DIR"
      cat > "$CONFIG_FILE" <<EOF
# Fiochat system config template
# Edit this file and then restart services.

model: openai:gpt-4o-mini
clients:
- type: openai
  api_key: YOUR_API_KEY_HERE

save: true
save_session: null

# telegram:
#   telegram_bot_token: YOUR_BOT_TOKEN_HERE
#   allowed_user_ids: "123456789"
#   server_name: myserver
#   ai_service_api_url: http://127.0.0.1:8000/v1/chat/completions
#   ai_service_model: default
#   ai_service_auth_token: Bearer <no-auth>
EOF
      ok "✓ Template created at: ${CONFIG_FILE}"
      ;;
    *)
      cfg_source="2"
      ;;
  esac

  echo ""
  echo -e "${BLUE}Installing systemd services${NC}"
  sudo cp "${systemd_dir}/fiochat.service" /etc/systemd/system/
  sudo cp "${systemd_dir}/fiochat-telegram.service" /etc/systemd/system/
  ok "✓ Installed unit files to /etc/systemd/system/"

  if [[ -n "$use_current_user" ]]; then
    echo ""
    echo -e "${BLUE}Creating drop-in overrides for user: ${service_user}${NC}"
    sudo mkdir -p /etc/systemd/system/fiochat.service.d
    sudo mkdir -p /etc/systemd/system/fiochat-telegram.service.d

    sudo tee /etc/systemd/system/fiochat.service.d/override.conf >/dev/null <<EOF
[Service]
User=${service_user}
Group=${service_user}
EOF

    sudo tee /etc/systemd/system/fiochat-telegram.service.d/override.conf >/dev/null <<EOF
[Service]
User=${service_user}
Group=${service_user}
EOF
    ok "✓ Drop-ins created"
  fi

  echo ""
  echo -e "${BLUE}Installing config to /etc/fiochat/config.yaml${NC}"
  sudo mkdir -p /etc/fiochat
  sudo cp "${CONFIG_FILE}" /etc/fiochat/config.yaml
  # Prefer root:service_user; fallback to root:root if group doesn't exist
  sudo chown "root:${service_user}" /etc/fiochat/config.yaml 2>/dev/null || sudo chown root:root /etc/fiochat/config.yaml
  sudo chmod 640 /etc/fiochat/config.yaml
  ok "✓ Installed: /etc/fiochat/config.yaml"

  echo ""
  echo -e "${BLUE}Creating state directory /var/lib/fiochat${NC}"
  sudo mkdir -p /var/lib/fiochat
  sudo chown -R "${service_user}:${service_user}" /var/lib/fiochat 2>/dev/null || true
  sudo chmod 750 /var/lib/fiochat
  ok "✓ Ready: /var/lib/fiochat"

  echo ""
  sudo systemctl daemon-reload
  ok "✓ systemd daemon reloaded"

  echo ""
  if [[ -n "$release_installed" ]]; then
    echo -e "${BLUE}Starting services now${NC}"

    # Determine ai service port (default 8000)
    port=""
    if [[ -n "${ai_service_url:-}" ]]; then
      port=$(echo "${ai_service_url}" | sed -nE 's|.*:([0-9]+).*|\1|p' || true)
    fi
    if [[ -z "${port}" && -f "${CONFIG_FILE}" ]]; then
      # try to read from config file
      ai_service_url_file=$(awk -F": " '/ai_service_api_url:/{print $2; exit}' "${CONFIG_FILE}" 2>/dev/null || true)
      ai_service_url_file=${ai_service_url_file#\"}
      ai_service_url_file=${ai_service_url_file%\"}
      port=$(echo "${ai_service_url_file}" | sed -nE 's|.*:([0-9]+).*|\1|p' || true)
    fi
    port=${port:-8000}

    if check_port_in_use "${port}" >/dev/null 2>&1; then
      echo ""
      warn "Port ${port} appears to be in use. This may conflict with fiochat's default API port."
      echo "Process info:" >&2
      check_port_in_use "${port}" | sed 's/^/  /' >&2 || true
      if ! prompt_yesno "Continue and start services anyway?" "N"; then
        err "Aborting service start. Resolve the port conflict or change the AI service port in /etc/fiochat/config.yaml and re-run."
        exit 1
      fi
    fi

    sudo systemctl enable --now fiochat.service fiochat-telegram.service
    ok "✓ Services enabled and started"
  else
    if prompt_yesno "Enable services on boot?" "Y"; then
      sudo systemctl enable fiochat.service fiochat-telegram.service
      ok "✓ Services enabled"
    fi
    warn "Services installed. Start them after you deploy binaries:"
    echo -e "  ${GREEN}sudo systemctl start fiochat.service fiochat-telegram.service${NC}"
  fi

  echo ""
  hr
  ok "✓ Production install complete"
  hr
  echo ""
  echo -e "${YELLOW}Verify:${NC}"
  echo -e "  ${GREEN}sudo systemctl status fiochat.service fiochat-telegram.service${NC}"
  echo -e "${YELLOW}Logs:${NC}"
  echo -e "  ${GREEN}sudo journalctl -u fiochat.service -f${NC}"
  echo -e "  ${GREEN}sudo journalctl -u fiochat-telegram.service -f${NC}"
  echo ""
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------
main() {
  title
  echo ""

  # Suggest a default mode
  local default_mode="1"
  if [[ "$OS_NAME" == "linux" && -d "/etc/systemd/system" ]]; then
    default_mode="2"
  fi

  echo -e "${BLUE}What are we doing today?${NC}"
  echo "  1) Development setup (local run)"
  echo "  2) Production install (Linux + systemd)"
  echo "  3) Inspect / verify existing configuration"
  echo ""
  echo -e "${YELLOW}Tip:${NC} Type ${YELLOW}q${NC} to exit."
  echo ""
  local mode
  mode="$(validated_choice "Choose" "${default_mode}" "1 2 3")" || exit 0

  case "$mode" in
    1) dev_journey ;;
    2) production_journey ;;
    3) inspect_only ;;
  esac
}

main "$@"
