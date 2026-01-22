#!/usr/bin/env bash
# Interactive configuration setup for fiochat
set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Fiochat Configuration Setup Wizard    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""

# Function to prompt for input with default
prompt_input() {
    local prompt="$1"
    local default="$2"
    local value

    if [[ -n "$default" ]]; then
        read -p "$(echo -e "${GREEN}${prompt}${NC} [${YELLOW}${default}${NC}]: ")" value
        echo "${value:-$default}"
    else
        read -p "$(echo -e "${GREEN}${prompt}${NC}: ")" value
        echo "$value"
    fi
}

# Function to prompt for secret (no echo)
prompt_secret() {
    local prompt="$1"
    local value

    read -s -p "$(echo -e "${GREEN}${prompt}${NC}: ")" value
    echo "" # New line after hidden input
    echo "$value"
}

# Function to select LLM provider
select_provider() {
    echo -e "${BLUE}Select LLM Provider:${NC}"
    echo "  1) OpenAI"
    echo "  2) Anthropic Claude"
    echo "  3) Azure OpenAI"
    echo "  4) Ollama (local)"
    echo "  5) Other / Manual configuration"
    echo ""

    read -p "$(echo -e "${GREEN}Choose (1-5)${NC}: ")" choice

    case $choice in
        1) echo "openai" ;;
        2) echo "claude" ;;
        3) echo "azure-openai" ;;
        4) echo "ollama" ;;
        *) echo "manual" ;;
    esac
}

# =============================================================================
# Part 1: AI Service Configuration
# =============================================================================

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Part 1: AI Service Configuration${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

CONFIG_DIR="${HOME}/.config/fiochat"
CONFIG_FILE="${CONFIG_DIR}/config.yaml"

# Fallback: Check if aichat config exists but fiochat doesn't
AICHAT_CONFIG_DIR="${HOME}/.config/aichat"
AICHAT_CONFIG_FILE="${AICHAT_CONFIG_DIR}/config.yaml"

# Check for existing fiochat config
if [[ -f "$CONFIG_FILE" ]]; then
    echo -e "${YELLOW}⚠️  Config file already exists: ${CONFIG_FILE}${NC}"
    read -p "$(echo -e "${GREEN}Overwrite?${NC} (y/N): ")" overwrite
    if [[ "$overwrite" != "y" && "$overwrite" != "Y" ]]; then
        echo -e "${GREEN}✓ Keeping existing AI service config${NC}"
        SKIP_AI_CONFIG=1
    fi
# Check for legacy aichat config and offer migration
elif [[ -f "$AICHAT_CONFIG_FILE" ]]; then
    echo -e "${BLUE}Found legacy aichat config at: ${AICHAT_CONFIG_FILE}${NC}"
    read -p "$(echo -e "${GREEN}Copy it to fiochat config?${NC} (Y/n): ")" migrate
    if [[ "$migrate" != "n" && "$migrate" != "N" ]]; then
        mkdir -p "$CONFIG_DIR"
        cp "$AICHAT_CONFIG_FILE" "$CONFIG_FILE"
        echo -e "${GREEN}✓ Migrated config from aichat to fiochat${NC}"
        echo -e "${YELLOW}  Note: Your original aichat config is still at ${AICHAT_CONFIG_FILE}${NC}"
        SKIP_AI_CONFIG=1
    fi
fi

if [[ -z "$SKIP_AI_CONFIG" ]]; then
    mkdir -p "$CONFIG_DIR"

    provider=$(select_provider)
    echo ""

    case $provider in
        openai)
            echo -e "${YELLOW}OpenAI Setup${NC}"
            api_key=$(prompt_secret "Enter OpenAI API Key")
            model=$(prompt_input "Model name" "gpt-4o-mini")

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
            echo "" >> "$CONFIG_FILE"
            echo "# Telegram bot configuration will be added below" >> "$CONFIG_FILE"
            TELEGRAM_SECTION_NEEDED=1
            ;;

        claude)
            echo -e "${YELLOW}Anthropic Claude Setup${NC}"
            api_key=$(prompt_secret "Enter Anthropic API Key")
            model=$(prompt_input "Model name" "claude-3-5-sonnet-20241022")

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
            TELEGRAM_SECTION_NEEDED=1
            ;;

        azure-openai)
            echo -e "${YELLOW}Azure OpenAI Setup${NC}"
            api_base=$(prompt_input "Azure API Base URL" "https://YOUR_RESOURCE.openai.azure.com/")
            api_key=$(prompt_secret "Enter Azure API Key")
            model=$(prompt_input "Deployment name" "gpt-4o-mini")

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
            TELEGRAM_SECTION_NEEDED=1
            ;;

        ollama)
            echo -e "${YELLOW}Ollama Setup (Local)${NC}"
            api_base=$(prompt_input "Ollama API Base URL" "http://localhost:11434")
            model=$(prompt_input "Model name" "llama3.2")

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
            TELEGRAM_SECTION_NEEDED=1
            ;;

        manual)
            echo -e "${YELLOW}Creating minimal config template${NC}"
            cat > "$CONFIG_FILE" <<EOF
# Fiochat Configuration File
# This file contains both AI service and Telegram bot configuration.
# Edit this file to configure your LLM provider
# See: https://github.com/sigoden/aichat/wiki/Configuration-Guide

model: openai:gpt-4o-mini
clients:
- type: openai
  api_key: YOUR_API_KEY_HERE

save: true
save_session: null
EOF
            echo -e "${YELLOW}⚠️  Please edit ${CONFIG_FILE} manually${NC}"
            TELEGRAM_SECTION_NEEDED=1
            ;;
    esac

    echo ""
    echo -e "${GREEN}✓ AI service config created: ${CONFIG_FILE}${NC}"
fi

# =============================================================================
# Part 2: Telegram Bot Configuration
# =============================================================================

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Part 2: Telegram Bot Configuration${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Only configure Telegram if we created a new AI config
if [[ -n "$TELEGRAM_SECTION_NEEDED" ]]; then
    echo -e "${YELLOW}Get your bot token from @BotFather on Telegram${NC}"
    echo -e "${YELLOW}Get your user ID from @userinfobot on Telegram${NC}"
    echo ""

    bot_token=$(prompt_secret "Telegram Bot Token")
    user_ids=$(prompt_input "Allowed User IDs (comma-separated)" "")
    server_name=$(prompt_input "Server Name" "$(hostname -s)")

    # Optional advanced settings
    echo ""
    read -p "$(echo -e "${GREEN}Configure advanced settings?${NC} (y/N): ")" advanced

    if [[ "$advanced" == "y" || "$advanced" == "Y" ]]; then
        ai_service_url=$(prompt_input "AI Service URL" "http://127.0.0.1:8000/v1/chat/completions")
        ai_service_model=$(prompt_input "AI Service Model" "default")
        ai_service_token=$(prompt_input "AI Service Auth Token" "Bearer dummy")
    else
        ai_service_url="http://127.0.0.1:8000/v1/chat/completions"
        ai_service_model="default"
        ai_service_token="Bearer dummy"
    fi

    # Append telegram section to the config.yaml file
    cat >> "$CONFIG_FILE" <<EOF

# ==============================================================================
# Telegram Bot Configuration
# ==============================================================================
# Get bot token from @BotFather: https://t.me/BotFather
# Get your user ID from @userinfobot: https://t.me/userinfobot
telegram:
  telegram_bot_token: ${bot_token}
  allowed_user_ids: "${user_ids}"
  server_name: ${server_name}
  ai_service_api_url: ${ai_service_url}
  ai_service_model: ${ai_service_model}
  ai_service_auth_token: ${ai_service_token}
  ai_service_session_namespace: ${server_name}
EOF

    echo ""
    echo -e "${GREEN}✓ Telegram bot config added to: ${CONFIG_FILE}${NC}"
else
    echo -e "${GREEN}✓ Using existing AI service config${NC}"
    echo -e "${YELLOW}Note: Telegram bot settings can be configured in ${CONFIG_FILE}${NC}"
    echo -e "${YELLOW}      or via environment variables (see telegram/.env.example)${NC}"
fi

# =============================================================================
# Summary
# =============================================================================

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ Configuration Complete!${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${YELLOW}Configuration File:${NC}"
echo "  ${CONFIG_FILE}"
echo ""
echo -e "${YELLOW}What's configured:${NC}"
if [[ -n "$TELEGRAM_SECTION_NEEDED" ]]; then
    echo "  ✓ AI Service (LLM provider and API keys)"
    echo "  ✓ Telegram Bot (bot token and allowed users)"
else
    echo "  ✓ AI Service configuration loaded"
    echo "  ℹ Telegram Bot can be configured via environment variables"
fi
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Build: ${GREEN}make build${NC}"
echo "  2. Run AI service: ${GREEN}make dev-ai${NC} (Terminal 1)"
echo "  3. Run Telegram bot: ${GREEN}make dev-telegram${NC} (Terminal 2)"
echo "  4. Test by messaging your bot on Telegram"
echo ""
echo -e "${BLUE}Happy chatting! 🚀${NC}"
