#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="${LUNA_MANAGER_GATE_TEST_OUT:-$ROOT/out/luna-manager-gate-test}"
QEMU_TIMEOUT="${LUNA_ONEFILE_QEMU_TIMEOUT:-12s}"
NIRI_QEMU_TIMEOUT="${LUNA_NIRI_QEMU_TIMEOUT:-60s}"

for artifact in \
  "$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-device-manager" \
  "$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-volume-manager" \
  "$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-system-runtime" \
  "$ROOT/out/luna-real-init.efi" \
  "$ROOT/out/luna-real-init.init"; do
  [ -x "$artifact" ] || [ -f "$artifact" ] || {
    echo "missing manager-gate test artifact: $artifact" >&2
    exit 1
  }
done

run_case() {
  local scenario="${1:-positive}"
  local failure=""
  local interactive=0
  local niri=0
  local noctalia=0
  local name="$scenario"
  case "$scenario" in
    positive) name=positive ;;
    missing-volume-manager|no-available-volumes) failure="$scenario" ;;
    interactive-login) interactive=1 ;;
    niri-session) interactive=1; niri=1 ;;
    noctalia-session) interactive=1; niri=1; noctalia=1 ;;
    *) echo "unknown manager-gate scenario: $scenario" >&2; exit 2 ;;
  esac
  echo
  echo "=== Luna manager-gate case: ${name} ==="
  export LUNA_MANAGER_GATE_TEST=1
  export LUNA_MANAGER_GATE_FAILURE="$failure"
  export LUNA_MANAGER_GATE_INTERACTIVE_TEST="$interactive"
  export LUNA_NIRI_SESSION_TEST="$niri"
  export LUNA_NOCTALIA_SESSION_TEST="$noctalia"
  export LUNA_REAL_SYSTEM_RUNTIME=1
  if [ "$niri" = "1" ]; then
    export LUNA_ONEFILE_QEMU_TIMEOUT="$NIRI_QEMU_TIMEOUT"
  else
    export LUNA_ONEFILE_QEMU_TIMEOUT="$QEMU_TIMEOUT"
  fi
  export LUNA_ONEFILE_REAL_TEST_OUT="$OUT/$name"
  "$ROOT/test-real-init.sh"
}

case "${1:-all}" in
  all)
    run_case positive
    run_case missing-volume-manager
    run_case no-available-volumes
    run_case interactive-login
    run_case niri-session
    run_case noctalia-session
    ;;
  positive|interactive-login|niri-session|noctalia-session|missing-volume-manager|no-available-volumes)
    run_case "$1"
    ;;
  *)
    echo "usage: $0 [all|positive|missing-volume-manager|no-available-volumes|interactive-login|niri-session|noctalia-session]" >&2
    exit 2
    ;;
esac
