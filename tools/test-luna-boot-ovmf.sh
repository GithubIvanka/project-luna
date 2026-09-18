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
LUNA_TEST_KERNEL="${LUNA_TEST_KERNEL:-$REPO_ROOT/dist/kernel-alpha-p0/7.2.4/bzImage}"
LUNA_TEST_SQUASHFS="${LUNA_TEST_SQUASHFS:-$REPO_ROOT/dist/work/system-partition/images/luna-0.1.0.squashfs}"
export OVMF_CODE OVMF_VARS LUNA_TEST_KERNEL LUNA_TEST_SQUASHFS

for input in "$OVMF_CODE" "$OVMF_VARS" "$LUNA_TEST_KERNEL" "$LUNA_TEST_SQUASHFS"; do
    test -f "$input" || { echo "Ошибка: не найден входной файл $input." >&2; exit 1; }
done

cd "$REPO_ROOT"
bash tools/build-luna-boot.sh
bash boot/luna-boot/tests/ovmf/run.sh
