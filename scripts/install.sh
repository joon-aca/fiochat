#!/usr/bin/env bash
set -euo pipefail

REPO_DEFAULT="joon-aca/fiochat"
REF_DEFAULT="master" # installer script ref (branch/tag)

REPO="$REPO_DEFAULT"
REF="$REF_DEFAULT"
TAG="${FIOCHAT_INSTALL_TAG:-}"
YES=0
PHASE="wizard"
ANSWERS_FILE=""
MODE=""
INSTALL_METHOD=""
CONFIG_SOURCE=""
SERVICE_USER=""
START_SERVICES=""

usage() {
  cat <<'EOF_USAGE'
Fiochat installer

Usage:
  # Interactive wizard (backward compatible)
  curl -fsSL https://raw.githubusercontent.com/<OWNER>/<REPO>/<REF>/scripts/install.sh | bash

  # Validate/apply/verify with answers file
  curl -fsSL https://raw.githubusercontent.com/<OWNER>/<REPO>/<REF>/scripts/install.sh | \
    bash -s -- validate --answers /etc/fiochat/install.env --mode production --yes

Commands:
  wizard   Interactive setup wizard (default)
  validate Validate required inputs and environment
  apply    Run installation/configuration flow
  verify   Verify installation artifacts and service state

Options:
  --repo OWNER/REPO        GitHub repo (default: joon-aca/fiochat)
  --ref REF                Git ref for installer script (default: master)
  --tag vX.Y.Z             Release tag to install (required for non-interactive release installs)
  --answers FILE           Path to KEY=VALUE answers file
  --mode MODE              install mode: production | development | inspect
  --install-method METHOD  production install method: release | manual
  --config-source SOURCE   config source: existing | rebuild | template
  --service-user USER      service user: svc | current | <username>
  --start-services         start/enable services
  --no-start-services      skip start/enable services
  -y, --yes                Non-interactive mode (fail fast if required inputs are missing)
  -h, --help               Show help

Notes:
  - Answers file values are exported as environment variables.
  - CLI flags override answers-file values.
  - Default command is 'wizard' for compatibility with existing one-liners.
EOF_USAGE
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing command: $1" >&2
    exit 1
  }
}

load_answers_file() {
  local file="$1"
  [[ -z "$file" ]] && return 0

  if [[ ! -f "$file" ]]; then
    echo "Answers file not found: $file" >&2
    exit 1
  fi

  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%$'\r'}"
    [[ -z "${line//[[:space:]]/}" ]] && continue
    [[ "$line" =~ ^[[:space:]]*# ]] && continue

    if [[ "$line" =~ ^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
      local key="${BASH_REMATCH[1]}"
      local value="${BASH_REMATCH[2]}"

      # Trim leading whitespace in value
      value="${value#${value%%[![:space:]]*}}"

      # Strip optional matching quotes
      if [[ "$value" =~ ^\".*\"$ ]]; then
        value="${value:1:${#value}-2}"
      elif [[ "$value" =~ ^\'.*\'$ ]]; then
        value="${value:1:${#value}-2}"
      fi

      export "$key=$value"
    else
      echo "Skipping invalid answers-file line: $line" >&2
    fi
  done < "$file"
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    wizard|validate|apply|verify)
      PHASE="$1"
      shift
      ;;
  esac
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --ref)
      REF="$2"
      shift 2
      ;;
    --tag)
      TAG="$2"
      shift 2
      ;;
    --answers)
      ANSWERS_FILE="$2"
      shift 2
      ;;
    --mode)
      MODE="$2"
      shift 2
      ;;
    --install-method)
      INSTALL_METHOD="$2"
      shift 2
      ;;
    --config-source)
      CONFIG_SOURCE="$2"
      shift 2
      ;;
    --service-user)
      SERVICE_USER="$2"
      shift 2
      ;;
    --start-services)
      START_SERVICES="1"
      shift
      ;;
    --no-start-services)
      START_SERVICES="0"
      shift
      ;;
    -y|--yes|--non-interactive)
      YES=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

need_cmd mktemp
need_cmd chmod

DOWNLOAD() {
  local url="$1" out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$out" "$url"
  else
    echo "Need curl or wget" >&2
    exit 1
  fi
}

# Load answers first, then let explicit CLI flags override.
load_answers_file "$ANSWERS_FILE"

# Allow answers file / existing env to provide defaults when explicit CLI flags were not set.
if [[ -z "$TAG" && -n "${FIOCHAT_INSTALL_TAG:-}" ]]; then
  TAG="${FIOCHAT_INSTALL_TAG}"
fi
if [[ "$YES" -eq 0 ]]; then
  case "${FIOCHAT_INSTALL_YES:-0}" in
    1|y|Y|yes|YES|true|TRUE|on|ON) YES=1 ;;
  esac
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

SETUP_URL="https://raw.githubusercontent.com/${REPO}/${REF}/scripts/setup-config.sh"
SETUP_PATH="${tmpdir}/setup-config.sh"

echo "Downloading setup script:"
echo "  ${SETUP_URL}"
DOWNLOAD "$SETUP_URL" "$SETUP_PATH"
chmod +x "$SETUP_PATH"

export FIOCHAT_INSTALL_PHASE="$PHASE"
export FIOCHAT_INSTALL_YES="$YES"
export FIOCHAT_INSTALL_REPO="$REPO"
export FIOCHAT_INSTALL_TAG="$TAG"

if [[ -n "$ANSWERS_FILE" ]]; then
  export FIOCHAT_INSTALL_ANSWERS_FILE="$ANSWERS_FILE"
fi

if [[ -n "$MODE" ]]; then
  export FIOCHAT_INSTALL_MODE="$MODE"
fi
if [[ -n "$INSTALL_METHOD" ]]; then
  export FIOCHAT_INSTALL_METHOD="$INSTALL_METHOD"
fi
if [[ -n "$CONFIG_SOURCE" ]]; then
  export FIOCHAT_INSTALL_CONFIG_SOURCE="$CONFIG_SOURCE"
fi
if [[ -n "$SERVICE_USER" ]]; then
  export FIOCHAT_INSTALL_SERVICE_USER="$SERVICE_USER"
fi
if [[ -n "$START_SERVICES" ]]; then
  export FIOCHAT_INSTALL_START_SERVICES="$START_SERVICES"
fi

exec "$SETUP_PATH"
