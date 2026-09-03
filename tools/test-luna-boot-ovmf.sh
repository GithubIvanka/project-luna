#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

command -v qemu-system-x86_64 >/dev/null 2>&1 || { echo "Ошибка: не найден qemu-system-x86_64." >&2; exit 1; }
for tool in sgdisk mkfs.ext4 mkfs.fat mformat mmd mcopy dd; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Ошибка: не найден инструмент $tool." >&2; exit 1; }
done

: "${OVMF_CODE:?Укажите OVMF_CODE, например /usr/share/OVMF/OVMF_CODE_4M.fd}"
: "${OVMF_VARS:?Укажите OVMF_VARS, например /usr/share/OVMF/OVMF_VARS_4M.fd}"
: "${LUNA_TEST_KERNEL:?Укажите LUNA_TEST_KERNEL, путь к bzImage}"
: "${LUNA_TEST_INITRD:?Укажите LUNA_TEST_INITRD, путь к initramfs.img}"
: "${LUNA_TEST_SQUASHFS:?Укажите LUNA_TEST_SQUASHFS, путь к тестовому SquashFS System Image}"

cd "$REPO_ROOT"
bash tools/build-luna-boot.sh
bash boot/luna-boot/tests/ovmf/run.sh
