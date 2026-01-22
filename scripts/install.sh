#!/usr/bin/env bash
set -euo pipefail

REPO_DEFAULT="joon-aca/fiochat"
REF_DEFAULT="master"   # installer ref (branch/tag). Default keeps installer evolving.
TAG=""                 # optional release tag to install (vX.Y.Z)
REPO="$REPO_DEFAULT"
REF="$REF_DEFAULT"

usage() {
  cat <<'EOF'
Fiochat installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/<OWNER>/<REPO>/<REF>/scripts/install.sh | bash

Options:
  --repo OWNER/REPO     GitHub repo (default: joon-aca/fiochat)
  --ref  REF            Git ref for installer script (default: master)
  --tag  vX.Y.Z         Release tag to install (pin the release version)
  -y, --yes             Non-interactive where possible (best-effort; still may prompt for secrets)
  -h, --help            Show help

Notes:
  - Default behavior fetches the latest installer (master) and runs the setup wizard.
  - If you set --tag, the wizard should use that tag for the release download.
EOF
}

YES=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO="$2"; shift 2;;
    --ref)  REF="$2"; shift 2;;
    --tag)  TAG="$2"; shift 2;;
    -y|--yes) YES=1; shift;;
    -h|--help) usage; exit 0;;
    *) echo "Unknown arg: $1"; usage; exit 1;;
  esac
done

need_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "Missing command: $1" >&2; exit 1; }; }
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

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

SETUP_URL="https://raw.githubusercontent.com/${REPO}/${REF}/scripts/setup-config.sh"
SETUP_PATH="${tmpdir}/setup-config.sh"

echo "Downloading setup wizard:"
echo "  ${SETUP_URL}"
DOWNLOAD "$SETUP_URL" "$SETUP_PATH"
chmod +x "$SETUP_PATH"

# Pass a pinned release tag through env var (non-breaking: setup script can ignore)
export FIOCHAT_INSTALL_TAG="${TAG}"

# Optional: tell setup-config to default to release install path / skip some prompts later if you add flags
export FIOCHAT_INSTALL_YES="${YES}"

exec "$SETUP_PATH"
