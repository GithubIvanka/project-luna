#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="${LUNA_ONEFILE_DISK_TEST_OUT:-${ROOT}/out/luna-sys-test}"
IMAGE="${LUNA_ONEFILE_IMAGE:-${ROOT}/out/luna-onefile.efi}"
OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
LOG="$OUT/serial.log"
[ -f "$IMAGE" ] || { echo "missing one-file kernel: $IMAGE" >&2; exit 1; }
[ -f "$OVMF_CODE" ] || { echo "missing OVMF_CODE: $OVMF_CODE" >&2; exit 1; }
[ -f "$OVMF_VARS" ] || { echo "missing OVMF_VARS: $OVMF_VARS" >&2; exit 1; }
command -v cargo >/dev/null
command -v qemu-system-x86_64 >/dev/null
command -v sgdisk >/dev/null
command -v mkfs.ext4 >/dev/null
command -v mkfs.btrfs >/dev/null
command -v mkfs.fat >/dev/null
command -v mcopy >/dev/null
command -v mmd >/dev/null
command -v mksquashfs >/dev/null
command -v dd >/dev/null
command -v truncate >/dev/null
rm -rf "$OUT"
mkdir -p "$OUT/esp/EFI/BOOT" "$OUT/system-root/images" "$OUT/system-root/cores" "$OUT/system-root/kernels/test" "$OUT/system-root/config" "$OUT/minimal-root"
echo "[1/4] Building luna-boot"
cargo build --manifest-path "$REPO_ROOT/components/boot/Cargo.toml" --release --target x86_64-unknown-uefi >/dev/null
EFI="$REPO_ROOT/components/boot/target/x86_64-unknown-uefi/release/luna-boot.efi"
cp "$EFI" "$OUT/esp/EFI/BOOT/BOOTX64.EFI"
echo "[2/4] Preparing minimal LUNA-SYS"
cp "$IMAGE" "$OUT/system-root/kernels/test/bzImage"
cp "$ROOT/out/rootfs/init" "$OUT/system-root/cores/luna-0.1.0.init"
printf "%s\n" "[init]" "name = \"luna-init\"" "version = \"0.1.0\"" "" "[architecture]" "arch = \"x86_64\"" "" "[kernels]" "compatible = [\"test\"]" > "$OUT/system-root/cores/luna-0.1.0.toml"
printf "%s\n" "[image]" "name = \"luna\"" "version = \"0.1.0\"" "format = \"squashfs\"" "role = \"normal\"" "" "[architecture]" "arch = \"x86_64\"" "" "[init]" "compatible = [\"0.1.0\"]" > "$OUT/system-root/images/luna-0.1.0.toml"
mksquashfs "$OUT/minimal-root" "$OUT/system-root/images/luna-0.1.0.squashfs" -noappend -quiet
echo "[3/4] Creating canonical GPT fixture"
truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 -n EFI "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/BOOT
mcopy -i "$OUT/esp.img" "$OUT/esp/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
mkfs.ext4 -q -F -L LUNA-SYS -d "$OUT/system-root" "$OUT/system.img" 192M
mkdir -p "$OUT/data-root/system" "$OUT/data-root/users/luna"
truncate -s 64M "$OUT/data.img"
mkfs.btrfs -q -f -L LUNA-DATA --rootdir "$OUT/data-root" "$OUT/data.img"
truncate -s 360M "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D --partition-guid=3:7A6D5A7A-0003-4C55-4E41-444154412020 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+64M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none
dd if="$OUT/data.img" of="$OUT/disk.img" bs=512 seek=526336 conv=notrunc status=none
cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"
echo "[4/4] Booting UEFI fixture"
set +e
timeout 15s qemu-system-x86_64 -machine q35 -m 512M -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" -drive format=raw,file="$OUT/disk.img" -serial "file:$LOG" -display none -no-reboot -no-shutdown >"$OUT/qemu.log" 2>&1
status=$?
set -e
if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then echo "QEMU failed with status $status" >&2; exit "$status"; fi
grep -Fq "Run /init as init process" "$LOG"
grep -Fq "LUNA-INIT: PID 1" "$LOG"
grep -Fq "LUNA-READY" "$LOG"
if grep -Eq "Kernel panic|general protection|No working init|Failed to execute" "$LOG"; then echo "fatal boot error detected" >&2; exit 1; fi
echo "Canonical LUNA-SYS fixture acceptance passed: UEFI -> luna-boot -> LUNA-SYS/LUNA-DATA -> LUNA-READY"