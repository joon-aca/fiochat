#!/usr/bin/env bash
set -euo pipefail

BOT_DIR="${BOT_DIR:-/opt/fiochat}"

die() { echo "ERROR: $*" >&2; exit 1; }
note() { echo "==> $*"; }

require_root() {
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "must run as root (sudo)."
  fi
}

main() {
  require_root

  [[ -d "$BOT_DIR" ]] || die "BOT_DIR not found: $BOT_DIR"
  [[ -x "$BOT_DIR/deploy/scripts/fio-validate-healthchecks.sh" ]] || die "missing: $BOT_DIR/deploy/scripts/fio-validate-healthchecks.sh"

  note "Repo: $BOT_DIR"
  note "RUN_ONESHOT=${RUN_ONESHOT:-0}"
  note "FIO_NOTIFY_ENV_FILE=${FIO_NOTIFY_ENV_FILE:-/etc/fiochat/notify.env}"

  if command -v shellcheck >/dev/null 2>&1; then
    note "shellcheck (best-effort)"
    shellcheck \
      "$BOT_DIR/deploy/scripts/fio-install-healthchecks.sh" \
      "$BOT_DIR/deploy/scripts/fio-validate-healthchecks.sh" \
      "$BOT_DIR/deploy/scripts/fio-run-local-tests.sh" \
      || die "shellcheck failed"
  else
    note "shellcheck not installed; skipping"
  fi

  note "Running validator (end-to-end checks)"
  RUN_ONESHOT="${RUN_ONESHOT:-0}" \
    FIO_NOTIFY_ENV_FILE="${FIO_NOTIFY_ENV_FILE:-/etc/fiochat/notify.env}" \
    "$BOT_DIR/deploy/scripts/fio-validate-healthchecks.sh"

  note "Done"
}

main "$@"
