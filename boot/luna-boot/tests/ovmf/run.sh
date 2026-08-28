#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT_DIR}/tests/ovmf/out"
mkdir -p "$OUT"

: "${OVMF_CODE:?Set OVMF_CODE to OVMF_CODE.fd}"
: "${OVMF_VARS:?Set OVMF_VARS to a writable OVMF_VARS.fd copy}"
: "${LUNA_TEST_KERNEL:?Set LUNA_TEST_KERNEL to a Linux x86_64 bzImage}"
: "${LUNA_TEST_INITRD:?Set LUNA_TEST_INITRD to an initramfs image}"
: "${LUNA_TEST_SQUASHFS:?Set LUNA_TEST_SQUASHFS to a Luna SquashFS image}"

command -v cargo >/dev/null
command -v qemu-system-x86_64 >/dev/null
command -v sgdisk >/dev/null
command -v mkfs.ext4 >/dev/null
command -v mkfs.fat >/dev/null
command -v mcopy >/dev/null
command -v mmd >/dev/null
command -v mformat >/dev/null
command -v dd >/dev/null

cargo build --release --target x86_64-unknown-uefi --manifest-path "$ROOT_DIR/Cargo.toml"
EFI="$ROOT_DIR/target/x86_64-unknown-uefi/release/luna-boot.efi"

rm -f "$OUT"/disk.img "$OUT"/esp.img "$OUT"/system.img
truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/LUNA
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI

rm -rf "$OUT/system-root"
mkdir -p "$OUT/system-root/images" "$OUT/system-root/kernels/test"
cp "$LUNA_TEST_KERNEL" "$OUT/system-root/kernels/test/bzImage"
cp "$LUNA_TEST_INITRD" "$OUT/system-root/kernels/test/initramfs.img"
cp "$LUNA_TEST_SQUASHFS" "$OUT/system-root/images/luna-test.squashfs"
mkfs.ext4 -q -F -L LUNA-SYSTEM -d "$OUT/system-root" "$OUT/system.img" 256M

# GPT: ESP at 1 MiB, system at 65 MiB.
truncate -s 321M "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
sgdisk -n 1:2048:+64M -t 1:ef00 -c 1:EFI \
       -n 2:133120:+256M -t 2:8300 -c 2:system "$OUT/disk.img" >/dev/null

dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none

cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"
exec qemu-system-x86_64 \
  -machine q35 \
  -m 2G \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" \
  -drive format=raw,file="$OUT/disk.img" \
  -serial stdio \
  -no-reboot
