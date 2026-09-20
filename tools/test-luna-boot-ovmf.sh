#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

command -v qemu-system-x86_64 >/dev/null 2>&1 || { echo "Ошибка: не найден qemu-system-x86_64." >&2; exit 1; }
for tool in sgdisk mkfs.ext4 mkfs.fat mformat mmd mcopy dd; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Ошибка: не найден инструмент $tool." >&2; exit 1; }
done

# Keep the autonomous verification check self-contained on a normal Alpha dev host.
# Explicit environment overrides remain supported for non-default build artifacts.
OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
LUNA_TEST_KERNEL="${LUNA_TEST_KERNEL:-$REPO_ROOT/dist/kernel/current/bzImage}"
export OVMF_CODE OVMF_VARS LUNA_TEST_KERNEL

for input in "$OVMF_CODE" "$OVMF_VARS" "$LUNA_TEST_KERNEL"; do
    test -f "$input" || { echo "Ошибка: не найден входной файл $input." >&2; exit 1; }
done

cd "$REPO_ROOT"
bash tools/build-luna-boot.sh
SERIAL_LOG="$REPO_ROOT/boot/luna-boot/tests/ovmf/out/serial.log"
rm -f "$SERIAL_LOG"
set +e
LUNA_QEMU_SERIAL_LOG="$SERIAL_LOG" timeout 20s bash boot/luna-boot/tests/ovmf/run.sh
rc=$?
set -e
if [ "$rc" -ne 124 ]; then
    echo "Ошибка: OVMF test exited before timeout (status $rc)." >&2
    exit "$rc"
fi
grep -q "luna-system-runtime: boot success confirmed" "$SERIAL_LOG" \
    || { echo "Ошибка: boot success marker not found." >&2; exit 1; }
grep -q "luna-system-runtime: graphical UserSession launched /usr/bin/niri --session" "$SERIAL_LOG" \
    || { echo "Ошибка: graphical UserSession launch marker not found." >&2; exit 1; }
if grep -q "Kernel panic\|panicked at" "$SERIAL_LOG"; then
    echo "Ошибка: kernel/userspace panic detected in OVMF test." >&2
    exit 1
fi
if grep -q "greetd\|greeter" "$SERIAL_LOG"; then
    echo "Ошибка: legacy greetd/greeter path appeared in OVMF test output." >&2
    exit 1
fi
echo "OVMF verification: PASS"
