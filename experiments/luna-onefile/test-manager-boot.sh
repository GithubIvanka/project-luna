#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="$ROOT/out/luna-manager-gate-test"
QEMU_TIMEOUT="${LUNA_ONEFILE_QEMU_TIMEOUT:-12s}"

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
  local failure="${1:-}"
  local name="${failure:-positive}"
  echo
  echo "=== Luna manager-gate case: ${name} ==="
  export LUNA_MANAGER_GATE_TEST=1
  export LUNA_MANAGER_GATE_FAILURE="$failure"
  export LUNA_REAL_SYSTEM_RUNTIME=1
  export LUNA_ONEFILE_QEMU_TIMEOUT="$QEMU_TIMEOUT"
  export LUNA_ONEFILE_REAL_TEST_OUT="$OUT/$name"
  "$ROOT/test-real-init.sh"
}

case "${1:-all}" in
  all)
    run_case ""
    run_case "missing-volume-manager"
    run_case "no-available-volumes"
    ;;
  positive)
    run_case ""
    ;;
  missing-volume-manager|no-available-volumes)
    run_case "$1"
    ;;
  *)
    echo "usage: $0 [all|positive|missing-volume-manager|no-available-volumes]" >&2
    exit 2
    ;;
esac
