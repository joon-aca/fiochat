#!/usr/bin/env bash
set -euo pipefail

# Idempotent installer for fio health checks + notify pipeline (systemd timers).
# Assumes repo is present at /opt/fiochat (override with BOT_DIR).

BOT_DIR="${BOT_DIR:-/opt/fiochat}"
ENV_DIR="${ENV_DIR:-/etc/fiochat}"
NOTIFY_ENV_FILE="${FIO_NOTIFY_ENV_FILE:-$ENV_DIR/notify.env}"

die() { echo "ERROR: $*" >&2; exit 1; }

require_root() {
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "must run as root (sudo)."
  fi
}

ensure_env_dir() {
  mkdir -p "$ENV_DIR"
  chmod 700 "$ENV_DIR" || true
}

ensure_notify_env() {
  if [[ -f "$NOTIFY_ENV_FILE" ]]; then
    chmod 600 "$NOTIFY_ENV_FILE" || true
    return 0
  fi

  cat >"$NOTIFY_ENV_FILE" <<'EOF'
# fio-bot local notify endpoint (consumed by fio-notify + health scripts)
FIO_NOTIFY_BIND=127.0.0.1
FIO_NOTIFY_PORT=8787

# Replace with a random long secret
FIO_NOTIFY_SECRET=REPLACE_WITH_RANDOM_LONG_SECRET

# Telegram chat to post alerts to (must be a stable chat_id)
TELEGRAM_NOTIFY_CHAT_ID=REPLACE_WITH_CHAT_ID

# Optional: set to 0 if you only want DOWN alerts
NOTIFY_UP_TRANSITIONS=1
EOF
  chmod 600 "$NOTIFY_ENV_FILE"
}

install_bins() {
  install -m 0755 "$BOT_DIR/deploy/scripts/fio-notify" /usr/local/bin/fio-notify
  install -m 0755 "$BOT_DIR/deploy/scripts/fio-health-local.sh" /usr/local/bin/fio-health-local.sh
  install -m 0755 "$BOT_DIR/deploy/scripts/fio-health-porco.sh" /usr/local/bin/fio-health-porco.sh
}

install_units() {
  cp -f "$BOT_DIR/deploy/systemd/fio-health-local.service" /etc/systemd/system/
  cp -f "$BOT_DIR/deploy/systemd/fio-health-local.timer" /etc/systemd/system/
  cp -f "$BOT_DIR/deploy/systemd/fio-health-porco.service" /etc/systemd/system/
  cp -f "$BOT_DIR/deploy/systemd/fio-health-porco.timer" /etc/systemd/system/
}

enable_timers() {
  systemctl daemon-reload
  systemctl enable --now fio-health-local.timer
  systemctl enable --now fio-health-porco.timer
}

main() {
  require_root
  [[ -d "$BOT_DIR" ]] || die "BOT_DIR not found: $BOT_DIR"

  ensure_env_dir
  ensure_notify_env
  install_bins
  install_units
  enable_timers

  echo "OK: healthcheck scripts + systemd timers installed."
  echo "Next:"
  echo "  - edit: $NOTIFY_ENV_FILE"
  echo "  - ensure /opt/fiochat/.env has matching FIO_NOTIFY_* + TELEGRAM_NOTIFY_CHAT_ID"
  echo "  - restart: systemctl restart fio-telegram.service"
  echo "  - test: $BOT_DIR/deploy/scripts/fio-run-local-tests.sh"
}

main "$@"
