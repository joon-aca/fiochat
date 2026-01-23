#!/usr/bin/env bash
set -euo pipefail

ENV_FILE_DEFAULT="/etc/fiochat/notify.env"
ENV_FILE="${FIO_NOTIFY_ENV_FILE:-$ENV_FILE_DEFAULT}"

die() { echo "ERROR: $*" >&2; exit 1; }
note() { echo "==> $*"; }

require_root() {
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "must run as root (sudo)."
  fi
}

load_env() {
  [[ -f "$ENV_FILE" ]] || die "env file not found: $ENV_FILE"
  # shellcheck disable=SC1090
  source "$ENV_FILE"

  [[ -n "${FIO_NOTIFY_BIND:-}" ]] || die "FIO_NOTIFY_BIND missing in $ENV_FILE"
  [[ -n "${FIO_NOTIFY_PORT:-}" ]] || die "FIO_NOTIFY_PORT missing in $ENV_FILE"
  [[ -n "${FIO_NOTIFY_SECRET:-}" ]] || die "FIO_NOTIFY_SECRET missing in $ENV_FILE"
  [[ -n "${TELEGRAM_NOTIFY_CHAT_ID:-}" ]] || die "TELEGRAM_NOTIFY_CHAT_ID missing in $ENV_FILE"
}

check_bot_active() {
  note "Checking fio-telegram.service is active"
  systemctl is-active --quiet fio-telegram.service || die "fio-telegram.service is not active"
}

check_notify_http() {
  local url="http://${FIO_NOTIFY_BIND}:${FIO_NOTIFY_PORT}/notify"
  note "POST $url with x-fio-secret"
  curl -fsS "$url" \
    -H 'content-type: application/json' \
    -H "x-fio-secret: ${FIO_NOTIFY_SECRET}" \
    -d '{"severity":"info","title":"fio validate","message":"notify endpoint smoke test","host":"fio","tags":["health","validate"]}' \
    >/dev/null
}

check_fio_notify_helper() {
  command -v fio-notify >/dev/null || die "fio-notify not found (expected /usr/local/bin/fio-notify)"
  note "Running fio-notify helper"
  FIO_NOTIFY_ENV_FILE="$ENV_FILE" fio-notify info "fio validate" "fio-notify helper smoke test" "fio" "health,validate" >/dev/null
}

check_timers() {
  note "Checking timers enabled/active"
  systemctl is-enabled --quiet fio-health-local.timer || die "fio-health-local.timer not enabled"
  systemctl is-enabled --quiet fio-health-porco.timer || die "fio-health-porco.timer not enabled"
  systemctl is-active --quiet fio-health-local.timer || die "fio-health-local.timer not active"
  systemctl is-active --quiet fio-health-porco.timer || die "fio-health-porco.timer not active"
}

optional_oneshot() {
  if [[ "${RUN_ONESHOT:-0}" != "1" ]]; then
    note "Skipping one-shot runs (set RUN_ONESHOT=1)"
    return 0
  fi
  note "Triggering one-shot health checks"
  systemctl start fio-health-local.service
  systemctl start fio-health-porco.service
}

main() {
  require_root
  load_env
  check_bot_active
  check_notify_http
  check_fio_notify_helper
  check_timers
  optional_oneshot
  note "OK"
}

main "$@"
