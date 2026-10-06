#!/usr/bin/env bash
set -euo pipefail

# User-facing launcher for the Luna kernel + Alpine userspace test.
# Run this file directly from any working directory.
#
# The VM opens with a GTK window and the guest serial console is mirrored
# into the terminal. This keeps the graphical QEMU window available while
# making kernel/OpenRC/login output visible and interactive.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT_DIR}/tests/ovmf/out-alpine-luna-kernel"
ALPINE_CACHE="${ROOT_DIR}/../../experiments/cache/alpine-3.24.2-netboot"
LUNA_KERNEL="${LUNA_KERNEL_PATH:-${ROOT_DIR}/../../dist/kernel/current/bzImage}"

: "${OVMF_CODE:=/usr/share/OVMF/OVMF_CODE_4M.fd}"
: "${OVMF_VARS:=/usr/share/OVMF/OVMF_VARS_4M.fd}"

ALPINE_CACHE="$(cd "$ALPINE_CACHE" && pwd)"
ALPINE_INITRAMFS_SOURCE="$ALPINE_CACHE/initramfs-virt"
ALPINE_MEDIA="$ALPINE_CACHE/alpine-3.24.2-boot-media.fat"

# Prefer the current graphical Linux desktop session when Wayland is used.
if [[ -z "${DISPLAY:-}" && -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/wayland-0" ]]; then
    export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
    export GDK_BACKEND="${GDK_BACKEND:-wayland}"
fi

for tool in cargo qemu-system-x86_64 sgdisk mkfs.ext4 mkfs.fat mcopy mmd truncate dd; do
    command -v "$tool" >/dev/null || {
        echo "missing required tool: $tool" >&2
        exit 1
    }
done

for file in "$LUNA_KERNEL" "$ALPINE_INITRAMFS_SOURCE" "$ALPINE_MEDIA" "$OVMF_CODE" "$OVMF_VARS"; do
    [[ -s "$file" ]] || {
        echo "missing artifact: $file" >&2
        exit 1
    }
done

rm -rf "$OUT"
mkdir -p "$OUT/system-root/alpine" "$OUT/esp"

echo "==> Using Luna kernel: $LUNA_KERNEL"
echo "==> Building luna-boot..."
cp "$LUNA_KERNEL" "$OUT/system-root/alpine/luna-kernel"
cp "$ALPINE_INITRAMFS_SOURCE" "$OUT/system-root/alpine/initramfs-virt"

FEATURES=alpine-luna-kernel
if [[ -n "${LUNA_ALPINE_DEBUG:-}" ]]; then
    FEATURES="alpine-luna-kernel,alpine-debug"
fi

cargo build --release \
    --manifest-path "$ROOT_DIR/Cargo.toml" \
    --features "$FEATURES" \
    --target x86_64-unknown-uefi

EFI="$ROOT_DIR/target/x86_64-unknown-uefi/release/luna-boot.efi"

echo "==> Creating EFI system partition..."
truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/BOOT
mmd -i "$OUT/esp.img" ::/EFI/LUNA
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

echo "==> Creating LUNA-SYS..."
mkfs.ext4 -q -F -L LUNA-SYS -d "$OUT/system-root" "$OUT/system.img" 256M

echo "==> Creating GPT test disk..."
truncate -s 520M "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
sgdisk \
    --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B \
    --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 \
    --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D \
    -n 1:2048:+64M -t 1:ef00 -c 1:EFI \
    -n 2:133120:+256M -t 2:8300 -c 2:LUNA-SYS \
    "$OUT/disk.img" >/dev/null

dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none
cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"

SERIAL_LOG="${LUNA_QEMU_SERIAL_LOG:-$OUT/serial.log}"
QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-gtk}"

mkdir -p "$(dirname "$SERIAL_LOG")"

echo
echo "============================================================"
echo " Luna OS / Alpine / Luna kernel QEMU test"
echo "============================================================"
echo "QEMU display : $QEMU_DISPLAY"
echo "Serial log   : $SERIAL_LOG"
echo "Disk image   : $OUT/disk.img"
echo
echo "The QEMU graphics window will open now."
echo "Guest boot/login is rendered in the QEMU graphics window."
echo "Serial transcript is saved to: $SERIAL_LOG"
if [[ -n "${LUNA_QEMU_SERIAL_STDIO:-}" ]]; then
    echo "Host serial console: enabled (debug mode)"
fi
echo "============================================================"
echo

# The guest console is tty0, so normal boot/login interaction belongs in the
# QEMU graphics window. Serial output is saved to SERIAL_LOG by default.
#
# Set LUNA_QEMU_SERIAL_STDIO=1 when a live serial console in the host terminal
# is needed for debugging.
if [[ -n "${LUNA_QEMU_SERIAL_STDIO:-}" ]]; then
    SERIAL_ARGS=(-serial stdio)
else
    SERIAL_ARGS=(-serial "file:${SERIAL_LOG}")
fi

qemu-system-x86_64 \
    -name "Luna OS - Alpine on Luna Kernel" \
    -machine q35 \
    -boot order=c,menu=off,strict=on \
    -m 2G \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" \
    -drive format=raw,file="$OUT/disk.img" \
    -drive if=none,format=raw,file="$ALPINE_MEDIA",id=alpinemedia \
    -device ide-hd,drive=alpinemedia,bus=ide.1,unit=0,bootindex=-1 \
    "${SERIAL_ARGS[@]}" \
    -display "$QEMU_DISPLAY" \
    -no-reboot \
    -no-shutdown \
    -D "$OUT/qemu.log"

QEMU_STATUS="$?"
echo
echo "QEMU exited with status $QEMU_STATUS"
echo "Serial log: $SERIAL_LOG"
exit "$QEMU_STATUS"
