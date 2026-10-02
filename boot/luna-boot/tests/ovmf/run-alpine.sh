#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT_DIR}/tests/ovmf/out-alpine"
ALPINE_CACHE="${ROOT_DIR}/../../experiments/cache/alpine-3.24.2-netboot"

: "${OVMF_CODE:=/usr/share/OVMF/OVMF_CODE_4M.fd}"
: "${OVMF_VARS:=/usr/share/OVMF/OVMF_VARS_4M.fd}"

ALPINE_CACHE="$(cd "$ALPINE_CACHE" && pwd)"
ALPINE_KERNEL="$ALPINE_CACHE/vmlinuz-virt"
ALPINE_INITRAMFS_SOURCE="$ALPINE_CACHE/initramfs-virt"
ALPINE_MODLOOP="$ALPINE_CACHE/modloop-virt"
ALPINE_INITRAMFS="$OUT/initramfs-virt-with-storage"
ALPINE_MEDIA="$ALPINE_CACHE/alpine-3.24.2-boot-media.fat"

if [[ -z "${DISPLAY:-}" && -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/wayland-0" ]]; then
    export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
fi

for tool in cargo qemu-system-x86_64 sgdisk mkfs.ext4 mcopy mmd truncate dd unsquashfs depmod cpio gzip; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
for file in "$ALPINE_KERNEL" "$ALPINE_INITRAMFS_SOURCE" "$ALPINE_MODLOOP" "$ALPINE_MEDIA"; do
    [ -s "$file" ] || { echo "missing Alpine artifact: $file" >&2; exit 1; }
done

rm -rf "$OUT"
mkdir -p "$OUT/system-root/alpine" "$OUT/esp"
cp "$ALPINE_KERNEL" "$OUT/system-root/alpine/vmlinuz-virt"

# Alpine virt keeps several storage drivers in modloop-virt. Add sd_mod to the
# initramfs so the control run is independent from an external modloop step.
INITRAMFS_ROOT="$OUT/initramfs-root"
rm -rf "$INITRAMFS_ROOT" "$ALPINE_INITRAMFS"
mkdir -p "$INITRAMFS_ROOT"
gzip -dc "$ALPINE_INITRAMFS_SOURCE" | (cd "$INITRAMFS_ROOT" && cpio -idm --quiet)
MODLOOP_ROOT="$OUT/modloop-root"
rm -rf "$MODLOOP_ROOT"
unsquashfs -d "$MODLOOP_ROOT" "$ALPINE_MODLOOP" >/dev/null
MODULE_DIR="$INITRAMFS_ROOT/lib/modules/6.18.52-0-virt"
mkdir -p "$MODULE_DIR/kernel/drivers/scsi"
cp -a "$MODLOOP_ROOT/modules/6.18.52-0-virt/kernel/drivers/scsi/sd_mod.ko" "$MODULE_DIR/kernel/drivers/scsi/"
depmod -b "$INITRAMFS_ROOT" 6.18.52-0-virt
( cd "$INITRAMFS_ROOT" && find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 > "$ALPINE_INITRAMFS" )
cp "$ALPINE_INITRAMFS" "$OUT/system-root/alpine/initramfs-virt"

FEATURES=alpine-test
if [[ -n "${LUNA_ALPINE_DEBUG:-}" ]]; then
    FEATURES=alpine-debug
fi
cargo build --release \
    --manifest-path "$ROOT_DIR/Cargo.toml" \
    --features "$FEATURES" \
    --target x86_64-unknown-uefi
EFI="$ROOT_DIR/target/x86_64-unknown-uefi/release/luna-boot.efi"

truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/BOOT
mmd -i "$OUT/esp.img" ::/EFI/LUNA
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

mkfs.ext4 -q -F -L LUNA-SYS -d "$OUT/system-root" "$OUT/system.img" 256M

truncate -s 520M "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
sgdisk \
    --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B \
    --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 \
    --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D \
    -n 1:2048:+64M -t 1:ef00 -c 1:EFI \
    -n 2:133120:+256M -t 2:8300 -c 2:LUNA-SYS "$OUT/disk.img" >/dev/null

dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none

cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"
SERIAL_LOG="${LUNA_QEMU_SERIAL_LOG:-$OUT/serial.log}"
mkdir -p "$(dirname "$SERIAL_LOG")"
exec qemu-system-x86_64 \
    -machine q35 \
    -boot order=c,menu=off,strict=on \
    -m 2G \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" \
    -drive format=raw,file="$OUT/disk.img" \
    -drive if=none,format=raw,file="$ALPINE_MEDIA",id=alpinemedia \
    -device ide-hd,drive=alpinemedia,bus=ide.1,unit=0,bootindex=-1 \
    -serial "file:${SERIAL_LOG}" \
    -display "${LUNA_QEMU_DISPLAY:-gtk}" \
    -no-reboot \
    -no-shutdown \
    -D "$OUT/qemu.log"
