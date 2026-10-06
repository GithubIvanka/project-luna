#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
LOG="${LUNA_ONEFILE_UEFI_LOG:-${ROOT}/out/qemu-stage1-uefi.log}"
OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
KERNEL="${LUNA_ONEFILE_IMAGE:-${ROOT}/out/luna-onefile.efi}"

[ -f "$KERNEL" ] || { echo "missing one-file kernel: $KERNEL" >&2; exit 1; }

mkdir -p "$(dirname "$LOG")"
rm -f "$LOG"

set +e
timeout 25s env \
    OVMF_CODE="$OVMF_CODE" \
    OVMF_VARS="$OVMF_VARS" \
    LUNA_TEST_KERNEL="$KERNEL" \
    LUNA_QEMU_DISPLAY=none \
    LUNA_QEMU_SERIAL_LOG="$LOG" \
    bash "$REPO_ROOT/components/boot/tests/ovmf/run.sh"
status=$?
set -e

if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
    echo "UEFI boot harness failed with status $status" >&2
    exit "$status"
fi

grep -Fq 'Run /init as init process' "$LOG"
grep -Fq 'LUNA-INIT: PID 1' "$LOG"
grep -Fq 'LUNA-READY' "$LOG"

if grep -Eq 'Kernel panic|general protection|No working init|Failed to execute' "$LOG"; then
    echo "UEFI Stage 1 acceptance failed; fatal boot error detected." >&2
    exit 1
fi

echo "UEFI Stage 1 acceptance passed: UEFI -> luna-boot -> Linux -> LUNA-READY"