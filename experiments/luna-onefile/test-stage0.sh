#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMAGE="${LUNA_ONEFILE_IMAGE:-${ROOT}/out/luna-onefile.efi}"
LOG="${LUNA_ONEFILE_TEST_LOG:-${ROOT}/out/qemu-stage0.log}"

[ -f "$IMAGE" ] || {
    echo "missing Stage 0 image: $IMAGE" >&2
    exit 1
}

mkdir -p "$(dirname "$LOG")"

set +e
timeout 8s qemu-system-x86_64 \
    -m 128M \
    -nographic \
    -no-reboot \
    -no-shutdown \
    -kernel "$IMAGE" \
    >"$LOG" 2>&1
status=$?
set -e

# The timeout is expected because luna-init intentionally remains PID 1.
if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
    echo "QEMU failed with status $status" >&2
    exit "$status"
fi

grep -Fq 'Run /init as init process' "$LOG"
grep -Fq 'LUNA-INIT: PID 1' "$LOG"
grep -Fq 'LUNA-READY' "$LOG"

if grep -Eq 'Kernel panic|general protection|No working init|Failed to execute' "$LOG"; then
    echo "Stage 0 acceptance failed; fatal boot error detected." >&2
    exit 1
fi

echo "Stage 0 acceptance passed: LUNA-READY"