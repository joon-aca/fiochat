#!/usr/bin/env bash
# Interactive configuration setup for fiochat
set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Initialize variables for set -u compatibility
SKIP_AI_CONFIG="${SKIP_AI_CONFIG:-}"
TELEGRAM_SECTION_NEEDED="${TELEGRAM_SECTION_NEEDED:-}"
TEST_MODE="${TEST_MODE:-}"

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
        echo -e "${YELLOW}AI Service Auth Token (press Enter for default 'Bearer dummy'):${NC}"
        ai_service_token=$(prompt_secret "AI Service Auth Token")
        ai_service_token="${ai_service_token:-Bearer dummy}"
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
# Release Installer Helper Functions
# =============================================================================

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) echo "amd64" ;;
    aarch64|arm64) echo "arm64" ;;
    *)
      echo -e "${RED}✗ Unsupported architecture: ${arch}${NC}" >&2
      exit 1
      ;;
  esac
}

need_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || {
    echo -e "${RED}✗ Missing required command: ${cmd}${NC}"
    exit 1
  }
}

download_file() {
  local url="$1"
  local out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$out" "$url"
  else
    echo -e "${RED}✗ Need curl or wget to download releases${NC}"
    exit 1
  fi
}

verify_sha256() {
  local sha_file="$1"
  local tar_file="$2"

  need_cmd sha256sum
  # sha file must contain "<sha>  <filename>"
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

  echo -e "${BLUE}Downloading release:${NC} ${tar_url}"
  download_file "$tar_url" "${tmpdir}/${base}"

  echo -e "${BLUE}Downloading checksum:${NC} ${sha_url}"
  download_file "$sha_url" "${tmpdir}/${base}.sha256"

  echo -e "${BLUE}Verifying checksum...${NC}"
  (cd "$tmpdir" && verify_sha256 "${base}.sha256" "${base}")
  echo -e "${GREEN}✓ Checksum OK${NC}"

  echo -e "${BLUE}Extracting...${NC}"
  tar -xzf "${tmpdir}/${base}" -C "$tmpdir"

  # Expect the tarball to contain a single top-level dir.
  local extracted_dir
  extracted_dir="$(find "$tmpdir" -maxdepth 1 -type d -name "fiochat-*" | head -n 1)"
  if [[ -z "$extracted_dir" ]]; then
    echo -e "${RED}✗ Could not find extracted fiochat directory in tarball${NC}"
    exit 1
  fi

  echo -e "${BLUE}Installing to /opt/fiochat...${NC}"
  sudo rm -rf /opt/fiochat
  sudo mkdir -p /opt/fiochat
  sudo cp -a "${extracted_dir}/." /opt/fiochat/

  # Install binary if present
  if [[ -f "/opt/fiochat/bin/fio" ]]; then
    echo -e "${BLUE}Installing fio binary to /usr/local/bin/fio...${NC}"
    sudo install -m 755 /opt/fiochat/bin/fio /usr/local/bin/fio
  elif [[ -f "/opt/fiochat/fio" ]]; then
    echo -e "${BLUE}Installing fio binary to /usr/local/bin/fio...${NC}"
    sudo install -m 755 /opt/fiochat/fio /usr/local/bin/fio
  else
    echo -e "${RED}✗ Release tarball missing fio binary (expected bin/fio or fio)${NC}"
    exit 1
  fi

  # Ensure /opt is root-owned
  sudo chown -R root:root /opt/fiochat
  sudo chmod -R go-w /opt/fiochat

  echo -e "${GREEN}✓ Release installed to /opt/fiochat${NC}"
}

# =============================================================================
# Part 3: systemd Service Installation (Optional)
# =============================================================================

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Part 3: systemd Service Installation${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Determine script and project directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SYSTEMD_DIR="${PROJECT_ROOT}/deploy/systemd"

# Initialize variables for set -u compatibility
SKIP_SYSTEMD="${SKIP_SYSTEMD:-}"
USE_CURRENT_USER="${USE_CURRENT_USER:-}"
USE_SYSTEM_CONFIG="${USE_SYSTEM_CONFIG:-}"
SYSTEM_CONFIG_INSTALLED="${SYSTEM_CONFIG_INSTALLED:-}"
SYSTEMD_INSTALLED="${SYSTEMD_INSTALLED:-}"
SERVICES_ENABLED="${SERVICES_ENABLED:-}"
SERVICE_USER="${SERVICE_USER:-}"

# Check if we're on a Linux system with systemd
if [[ ! -d "/etc/systemd/system" ]] && [[ -z "$TEST_MODE" ]]; then
    echo -e "${YELLOW}⚠️  systemd not detected on this system${NC}"
    echo -e "${YELLOW}   Skipping service installation${NC}"
    SKIP_SYSTEMD=1
fi

# Check if systemd service files exist
if [[ -z "$SKIP_SYSTEMD" ]] && [[ ! -d "$SYSTEMD_DIR" ]]; then
    echo -e "${RED}✗ systemd service files not found at: ${SYSTEMD_DIR}${NC}"
    SKIP_SYSTEMD=1
fi

if [[ -z "$SKIP_SYSTEMD" ]]; then
    echo -e "${YELLOW}Install systemd services for production deployment?${NC}"
    echo ""
    read -p "$(echo -e "${GREEN}Install systemd services?${NC} (y/N): ")" install_systemd

    if [[ "$install_systemd" == "y" || "$install_systemd" == "Y" ]]; then
        # Check for sudo
        if ! command -v sudo >/dev/null 2>&1; then
            echo -e "${RED}✗ sudo not found. Cannot install system services.${NC}"
        else
            echo ""

            # Ask which user should run the services
            CURRENT_USER="$(id -un)"
            echo -e "${BLUE}Which user should run the services?${NC}"
            echo "  1) ${CURRENT_USER} (current user)"
            echo "  2) svc (dedicated service user - will be created if missing)"
            echo ""
            read -p "$(echo -e "${GREEN}Choose (1-2)${NC} [1]: ")" user_choice

            if [[ "$user_choice" == "2" ]]; then
                SERVICE_USER="svc"
                # Create svc user automatically if it doesn't exist
                if ! id svc >/dev/null 2>&1; then
                    echo ""
                    echo -e "${BLUE}Creating service user 'svc'...${NC}"
                    sudo useradd -r -s /bin/false -d /var/lib/fiochat svc || {
                        echo -e "${RED}✗ Failed to create svc user${NC}"
                        exit 1
                    }
                    echo -e "${GREEN}✓ Created user svc${NC}"
                fi
            else
                USE_CURRENT_USER=1
                SERVICE_USER="${CURRENT_USER}"
            fi

            # For systemd installs, default to system config
            USE_SYSTEM_CONFIG=1
            echo ""
            echo -e "${YELLOW}Config will be installed to /etc/fiochat/config.yaml${NC}"
            echo -e "${YELLOW}(standard location for system services)${NC}"

            # ---- Release install option (default yes) --------------------
            echo ""
            read -p "$(echo -e "${GREEN}Install from GitHub Release?${NC} (Y/n): ")" install_release
            if [[ "$install_release" != "n" && "$install_release" != "N" ]]; then
                # version prompt (default from FIOCHAT_INSTALL_TAG env or v0.0.0)
                default_version="${FIOCHAT_INSTALL_TAG:-v0.0.0}"
                release_version=$(prompt_input "Release tag (e.g. v0.2.0)" "${default_version}")

                arch="$(detect_arch)"
                owner_repo="joon-aca/fiochat"

                # Preflight minimal deps
                need_cmd tar
                # sha256sum is used by verify_sha256()
                need_cmd sha256sum

                install_release_tarball "${owner_repo}" "${release_version}" "${arch}"
                RELEASE_INSTALLED=1
            else
                RELEASE_INSTALLED=""
            fi

            echo ""
            echo -e "${BLUE}Installing systemd services...${NC}"

            # Copy base service files
            sudo cp "${SYSTEMD_DIR}/fiochat.service" /etc/systemd/system/ || {
                echo -e "${RED}✗ Failed to copy fiochat.service${NC}"
                exit 1
            }
            echo -e "${GREEN}✓ Installed fiochat.service${NC}"

            sudo cp "${SYSTEMD_DIR}/fio-telegram.service" /etc/systemd/system/ || {
                echo -e "${RED}✗ Failed to copy fio-telegram.service${NC}"
                exit 1
            }
            echo -e "${GREEN}✓ Installed fio-telegram.service${NC}"

            # Create drop-ins if using current user (not svc)
            if [[ -n "$USE_CURRENT_USER" ]]; then
                echo ""
                echo -e "${BLUE}Creating systemd drop-in overrides for user ${SERVICE_USER}...${NC}"

                sudo mkdir -p /etc/systemd/system/fiochat.service.d
                sudo mkdir -p /etc/systemd/system/fio-telegram.service.d

                # Create drop-in for fiochat.service
                sudo tee /etc/systemd/system/fiochat.service.d/override.conf >/dev/null <<EOF
[Service]
User=${SERVICE_USER}
Group=${SERVICE_USER}
EOF
                echo -e "${GREEN}✓ Created fiochat.service drop-in${NC}"

                # Create drop-in for fio-telegram.service
                sudo tee /etc/systemd/system/fio-telegram.service.d/override.conf >/dev/null <<EOF
[Service]
User=${SERVICE_USER}
Group=${SERVICE_USER}
EOF
                echo -e "${GREEN}✓ Created fio-telegram.service drop-in${NC}"
            fi

            # Install system config
            echo ""
            echo -e "${BLUE}Installing system config...${NC}"
            sudo mkdir -p /etc/fiochat
            sudo cp "${CONFIG_FILE}" /etc/fiochat/config.yaml || {
                echo -e "${RED}✗ Failed to copy config${NC}"
                exit 1
            }
            # Config owned by root, readable by service group (standard Linux practice)
            sudo chown root:${SERVICE_USER} /etc/fiochat/config.yaml 2>/dev/null || \
                sudo chown root:root /etc/fiochat/config.yaml
            sudo chmod 640 /etc/fiochat/config.yaml
            echo -e "${GREEN}✓ Installed config to /etc/fiochat/config.yaml${NC}"
            SYSTEM_CONFIG_INSTALLED=1

            # Ensure state directory exists for runtime state
            echo ""
            echo -e "${BLUE}Creating state directory...${NC}"
            sudo mkdir -p /var/lib/fiochat
            sudo chown -R "${SERVICE_USER}:${SERVICE_USER}" /var/lib/fiochat 2>/dev/null || true
            sudo chmod 750 /var/lib/fiochat
            echo -e "${GREEN}✓ Created /var/lib/fiochat${NC}"

            # Reload systemd
            sudo systemctl daemon-reload || {
                echo -e "${RED}✗ Failed to reload systemd${NC}"
                exit 1
            }
            echo -e "${GREEN}✓ Reloaded systemd daemon${NC}"

            echo ""
            if [[ -n "${RELEASE_INSTALLED:-}" ]]; then
                echo -e "${BLUE}Starting services (release install path)...${NC}"
                sudo systemctl enable --now fiochat.service fio-telegram.service || {
                    echo -e "${RED}✗ Failed to enable/start services${NC}"
                    exit 1
                }
                echo -e "${GREEN}✓ Services enabled and started${NC}"
                SERVICES_ENABLED=1
            else
                read -p "$(echo -e "${GREEN}Enable services to start on boot?${NC} (y/N): ")" enable_services

                if [[ "$enable_services" == "y" || "$enable_services" == "Y" ]]; then
                    sudo systemctl enable fiochat.service fio-telegram.service || {
                        echo -e "${RED}✗ Failed to enable services${NC}"
                        exit 1
                    }
                    echo -e "${GREEN}✓ Services enabled on boot${NC}"
                    SERVICES_ENABLED=1
                fi

                echo ""
                echo -e "${YELLOW}Services installed (not started yet)${NC}"
            fi

            echo -e "${YELLOW}Running as user: ${SERVICE_USER}${NC}"
            echo -e "${YELLOW}Config location: /etc/fiochat/config.yaml${NC}"

            SYSTEMD_INSTALLED=1
        fi
    else
        echo -e "${YELLOW}⏭  Skipping systemd installation${NC}"
    fi
else
    echo -e "${YELLOW}⏭  Skipping systemd installation${NC}"
fi

# =============================================================================
# Summary
# =============================================================================

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ Configuration Complete!${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${YELLOW}Configuration Summary:${NC}"
if [[ -n "$SYSTEM_CONFIG_INSTALLED" ]]; then
    echo "  Config: /etc/fiochat/config.yaml"
else
    echo "  Config: ${CONFIG_FILE}"
fi
echo ""
echo -e "${YELLOW}What's configured:${NC}"
if [[ -n "$TELEGRAM_SECTION_NEEDED" ]]; then
    echo "  ✓ AI Service (LLM provider and API keys)"
    echo "  ✓ Telegram Bot (bot token and allowed users)"
else
    echo "  ✓ AI Service configuration loaded"
    echo "  ℹ Telegram Bot can be configured via environment variables"
fi
if [[ -n "$SYSTEMD_INSTALLED" ]]; then
    echo "  ✓ systemd services installed"
    if [[ -n "$USE_CURRENT_USER" ]]; then
        echo "    - Running as: ${SERVICE_USER}"
    else
        echo "    - Running as: svc (dedicated user)"
    fi
    if [[ -n "$SERVICES_ENABLED" ]]; then
        echo "    - Enabled on boot"
    fi
fi
echo ""
if [[ -n "$SYSTEMD_INSTALLED" ]]; then
    if [[ -n "${RELEASE_INSTALLED:-}" ]]; then
        echo -e "${YELLOW}Next Steps:${NC}"
        echo "  1. Test your bot on Telegram - it should be running now!"
        echo "  2. Check service status:"
        echo "     ${GREEN}sudo systemctl status fiochat.service fio-telegram.service${NC}"
        echo ""
        echo -e "${YELLOW}View logs:${NC}"
        echo "  ${GREEN}sudo journalctl -u fiochat.service -f${NC}"
        echo "  ${GREEN}sudo journalctl -u fio-telegram.service -f${NC}"
        echo ""
        echo -e "${YELLOW}Manage services:${NC}"
        echo "  ${GREEN}sudo systemctl stop fiochat.service fio-telegram.service${NC}"
        echo "  ${GREEN}sudo systemctl restart fiochat.service fio-telegram.service${NC}"
    else
        echo -e "${YELLOW}Next Steps (Build Locally):${NC}"
        echo "  1. Build: ${GREEN}make build${NC}"
        echo "  2. Install binary: ${GREEN}sudo make install${NC}"
        echo "  3. Build telegram: ${GREEN}cd telegram && npm run build${NC}"
        echo "  4. Deploy to /opt/fiochat (root-owned, read-only at runtime):"
        echo "     ${GREEN}sudo mkdir -p /opt/fiochat/telegram${NC}"
        echo "     ${GREEN}sudo cp -r telegram/dist telegram/package*.json /opt/fiochat/telegram/${NC}"
        echo "     ${GREEN}cd /opt/fiochat/telegram && sudo npm ci --production${NC}"
        echo "     ${GREEN}sudo chown -R root:root /opt/fiochat${NC}"
        echo "  5. Start services:"
        echo "     ${GREEN}sudo systemctl start fiochat.service fio-telegram.service${NC}"
        echo "  6. Check status:"
        echo "     ${GREEN}sudo systemctl status fiochat.service fio-telegram.service${NC}"
        echo ""
        echo -e "${YELLOW}View logs:${NC}"
        echo "  ${GREEN}sudo journalctl -u fiochat.service -f${NC}"
        echo "  ${GREEN}sudo journalctl -u fio-telegram.service -f${NC}"
    fi
else
    echo -e "${YELLOW}Next Steps (Development):${NC}"
    echo "  1. Build: ${GREEN}make build${NC}"
    echo "  2. Run AI service: ${GREEN}make dev-ai${NC} (Terminal 1)"
    echo "  3. Run Telegram bot: ${GREEN}make dev-telegram${NC} (Terminal 2)"
    echo "  4. Test by messaging your bot on Telegram"
fi
echo ""
echo -e "${BLUE}Happy chatting! 🚀${NC}"
