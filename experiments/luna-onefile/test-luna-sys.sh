#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="${LUNA_ONEFILE_DISK_TEST_OUT:-${ROOT}/out/luna-sys-test}"
IMAGE="${LUNA_ONEFILE_IMAGE:-${ROOT}/out/luna-onefile.efi}"
INIT="${LUNA_ONEFILE_INIT:-$ROOT/out/rootfs/init}"
SYSTEM_ROOT_SOURCE="${LUNA_ONEFILE_SYSTEM_ROOT:-}"
RECOVERY_ROOT_SOURCE="${LUNA_ONEFILE_RECOVERY_ROOT:-}"
RECOVERY_TEST="${LUNA_RECOVERY_TEST:-0}"
DATA_AMBIGUOUS_TEST="${LUNA_DATA_AMBIGUOUS_TEST:-0}"
DATA_CONFIG_TEST="${LUNA_DATA_CONFIG_TEST:-0}"
DATA_EXTERNAL_TEST="${LUNA_DATA_EXTERNAL_TEST:-0}"
OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
LOG="$OUT/serial.log"

[ -f "$IMAGE" ] || { echo "missing kernel image: $IMAGE" >&2; exit 1; }
[ -f "$INIT" ] || { echo "missing luna-init: $INIT" >&2; exit 1; }
[ -f "$OVMF_CODE" ] || { echo "missing OVMF_CODE: $OVMF_CODE" >&2; exit 1; }
[ -f "$OVMF_VARS" ] || { echo "missing OVMF_VARS: $OVMF_VARS" >&2; exit 1; }

for tool in cargo qemu-system-x86_64 sgdisk mkfs.ext4 mkfs.btrfs mkfs.fat mcopy mmd mksquashfs dd truncate; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

rm -rf "$OUT"
mkdir -p "$OUT/esp/EFI/BOOT" "$OUT/system-root/images" "$OUT/system-root/cores" "$OUT/system-root/kernels/test" "$OUT/system-root/config" "$OUT/system-root/recovery" "$OUT/minimal-root"
if [ "$RECOVERY_TEST" = "1" ]; then
  [ -n "$RECOVERY_ROOT_SOURCE" ] || { echo "missing Recovery DATA source root" >&2; exit 1; }
  [ -d "$RECOVERY_ROOT_SOURCE" ] || { echo "missing Recovery DATA source root: $RECOVERY_ROOT_SOURCE" >&2; exit 1; }
fi
if [ "$DATA_AMBIGUOUS_TEST" = "1" ] && [ "$RECOVERY_TEST" = "1" ]; then
  echo "Recovery test modes are mutually exclusive" >&2
  exit 1
fi
if [ "$DATA_CONFIG_TEST" = "1" ] && { [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_EXTERNAL_TEST" = "1" ]; }; then
  echo "DATA selection test modes are mutually exclusive" >&2
  exit 1
fi
if [ "$DATA_EXTERNAL_TEST" = "1" ] && [ "$RECOVERY_TEST" = "1" ]; then
  echo "DATA external and recovery test modes are mutually exclusive" >&2
  exit 1
fi
if [ "$DATA_EXTERNAL_TEST" = "1" ] && [ "$DATA_AMBIGUOUS_TEST" = "1" ]; then
  echo "DATA external and ambiguous test modes are mutually exclusive" >&2
  exit 1
fi

echo "[1/4] Building luna-boot"
cargo build --manifest-path "$REPO_ROOT/components/boot/Cargo.toml" --release --target x86_64-unknown-uefi >/dev/null
EFI="$REPO_ROOT/components/boot/target/x86_64-unknown-uefi/release/luna-boot.efi"
cp "$EFI" "$OUT/esp/EFI/BOOT/BOOTX64.EFI"

echo "[2/4] Preparing LUNA-SYS"
cp "$IMAGE" "$OUT/system-root/kernels/test/bzImage"
cp "$INIT" "$OUT/system-root/cores/luna-0.1.0.init"
printf "%s\n" "[init]" "name = \"luna-init\"" "version = \"0.1.0\"" "" "[architecture]" "arch = \"x86_64\"" "" "[kernels]" "compatible = [\"test\"]" > "$OUT/system-root/cores/luna-0.1.0.toml"
printf "%s\n" "[image]" "name = \"luna\"" "version = \"0.1.0\"" "format = \"squashfs\"" "role = \"normal\"" "" "[architecture]" "arch = \"x86_64\"" "" "[init]" "compatible = [\"0.1.0\"]" > "$OUT/system-root/images/luna-0.1.0.toml"

if [ -n "$SYSTEM_ROOT_SOURCE" ]; then
  [ -d "$SYSTEM_ROOT_SOURCE" ] || { echo "missing System Image source root: $SYSTEM_ROOT_SOURCE" >&2; exit 1; }
  cp -a "$SYSTEM_ROOT_SOURCE/." "$OUT/minimal-root/"
fi
mksquashfs "$OUT/minimal-root" "$OUT/system-root/images/luna-0.1.0.squashfs" -noappend -quiet
if [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ]; then
  mksquashfs "$RECOVERY_ROOT_SOURCE" "$OUT/system-root/recovery/recovery.squashfs" -noappend -quiet
  printf "%s\n" "[image]" "name = \"luna\"" "version = \"0.1.0\"" "format = \"squashfs\"" "role = \"recovery\"" "" "[architecture]" "arch = \"x86_64\"" "" "[init]" "compatible = [\"0.1.0\"]" > "$OUT/system-root/recovery/recovery.toml"
fi

echo "[3/4] Creating canonical GPT fixture"
truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 -n EFI "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/BOOT
mcopy -i "$OUT/esp.img" "$OUT/esp/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
if [ "$DATA_CONFIG_TEST" = "1" ]; then
  printf '%s\n' '[data]' 'preferred_disk_guid = "11111111-1111-4111-8111-111111111111"' 'preferred_partition_guid = "44444444-4444-4444-8444-444444444444"' > "$OUT/system-root/config/luna-data.toml"
elif [ "$DATA_EXTERNAL_TEST" = "1" ]; then
  printf '%s\n' '[data]' 'preferred_disk_guid = "66666666-6666-4666-8666-666666666666"' 'preferred_partition_guid = "77777777-7777-4777-8777-777777777777"' > "$OUT/system-root/config/luna-data.toml"
fi
mkfs.ext4 -q -F -L LUNA-SYS -d "$OUT/system-root" "$OUT/system.img" 192M

mkdir -p "$OUT/data-root/system" "$OUT/data-root/users/luna"
if [ -n "${LUNA_REAL_INIT:-}" ]; then
  mkdir -p "$OUT/data-root/system/libs" "$OUT/data-root/system/config"
  mkdir -p "$OUT/data-root/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
  mkdir -p "$OUT/data-root/system/apps"
fi
if [ "$RECOVERY_TEST" != "1" ]; then
  truncate -s 128M "$OUT/data.img"
  mkfs.btrfs -q -f -L LUNA-DATA --rootdir "$OUT/data-root" "$OUT/data.img"
  if [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
    cp "$OUT/data.img" "$OUT/data2.img"
  fi
fi

DISK_SIZE=448M
if [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_EXTERNAL_TEST" = "1" ]; then
  DISK_SIZE=320M
elif [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
  DISK_SIZE=576M
fi
truncate -s "$DISK_SIZE" "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
if [ "$RECOVERY_TEST" = "1" ]; then
  sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS "$OUT/disk.img" >/dev/null
elif [ "$DATA_EXTERNAL_TEST" = "1" ]; then
  sgdisk --disk-guid=11111111-1111-4111-8111-111111111111 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS "$OUT/disk.img" >/dev/null
  sgdisk --partition-guid=1:22222222-2222-4222-8222-222222222222 --partition-guid=2:33333333-3333-4333-8333-333333333333 "$OUT/disk.img" >/dev/null
elif [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
  sgdisk --disk-guid=11111111-1111-4111-8111-111111111111 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+128M -t 3:8300 -c 3:LUNA-DATA -n 4:788480:+128M -t 4:8300 -c 4:LUNA-DATA "$OUT/disk.img" >/dev/null
  sgdisk --partition-guid=1:22222222-2222-4222-8222-222222222222 --partition-guid=2:33333333-3333-4333-8333-333333333333 --partition-guid=3:44444444-4444-4444-8444-444444444444 --partition-guid=4:55555555-5555-4555-8555-555555555555 "$OUT/disk.img" >/dev/null
else
  sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D --partition-guid=3:7A6D5A7A-0003-4C55-4E41-444154412020 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+128M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
fi
dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none
if [ "$RECOVERY_TEST" != "1" ] && [ "$DATA_EXTERNAL_TEST" != "1" ]; then
  dd if="$OUT/data.img" of="$OUT/disk.img" bs=512 seek=526336 conv=notrunc status=none
  if [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
    dd if="$OUT/data2.img" of="$OUT/disk.img" bs=512 seek=788480 conv=notrunc status=none
  fi
fi

QEMU_EXTRA_DRIVES=()
if [ "$DATA_EXTERNAL_TEST" = "1" ]; then
  truncate -s 160M "$OUT/external-data-disk.img"
  sgdisk --zap-all "$OUT/external-data-disk.img" >/dev/null
  sgdisk --disk-guid=66666666-6666-4666-8666-666666666666 -n 1:2048:+128M -t 1:8300 -c 1:LUNA-DATA "$OUT/external-data-disk.img" >/dev/null
  sgdisk --partition-guid=1:77777777-7777-4777-8777-777777777777 "$OUT/external-data-disk.img" >/dev/null
  dd if="$OUT/data.img" of="$OUT/external-data-disk.img" bs=512 seek=2048 conv=notrunc status=none
  QEMU_EXTRA_DRIVES=(-drive "format=raw,file=$OUT/external-data-disk.img")
fi
cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"

echo "[4/4] Booting UEFI fixture"
set +e
timeout 15s qemu-system-x86_64 -machine q35 -m 512M -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" -drive format=raw,file="$OUT/disk.img" "${QEMU_EXTRA_DRIVES[@]}" -serial "file:$LOG" -display none -no-reboot -no-shutdown >"$OUT/qemu.log" 2>&1
status=$?
set -e
if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
  echo "QEMU failed with status $status" >&2
  exit "$status"
fi

if [ "${LUNA_DIRECT_INIT:-0}" = "1" ]; then
  grep -Fq "luna_boot: Luna: boot availability handoff=1 init=1" "$LOG"
  grep -Fq "luna_exec: Luna: preparing memory-resident luna-init for direct PID 1" "$LOG"
  grep -Fq "luna_exec: Luna: executing memory-resident luna-init as PID 1" "$LOG"
else
  grep -Fq "Run /init as init process" "$LOG"
fi

if [ "${LUNA_RECOVERY_TEST:-0}" = "1" ] || [ "${LUNA_DATA_AMBIGUOUS_TEST:-0}" = "1" ]; then
  grep -Fq "Luna: LUNA-DATA is unavailable; entering Recovery Environment" "$LOG"
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "mode=2" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "LUNA-SYSTEM-RUNTIME: READY" "$LOG"
fi

if [ "${LUNA_DATA_CONFIG_TEST:-0}" = "1" ]; then
  ! grep -Fq "Luna: LUNA-DATA is unavailable; entering Recovery Environment" "$LOG"
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "mode=0" "$LOG"
  grep -Fq "BTRFS: device label LUNA-DATA" "$LOG"
  grep -Fq "/dev/sda3" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "LUNA-SYSTEM-RUNTIME: READY" "$LOG"
fi

if [ "${LUNA_DATA_EXTERNAL_TEST:-0}" = "1" ]; then
  ! grep -Fq "Luna: LUNA-DATA is unavailable; entering Recovery Environment" "$LOG"
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "mode=0" "$LOG"
  grep -Fq "BTRFS: device label LUNA-DATA" "$LOG"
  grep -Fq "/dev/sdb1" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "LUNA-SYSTEM-RUNTIME: READY" "$LOG"
fi

if [ "${LUNA_REAL_INIT:-0}" = "1" ]; then
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "LUNA-SYSTEM-RUNTIME: READY" "$LOG"
else
  grep -Fq "LUNA-INIT: PID 1" "$LOG"
  grep -Fq "LUNA-READY" "$LOG"
fi

if grep -Eq "Kernel panic|general protection|No working init|Failed to execute" "$LOG"; then
  echo "fatal boot error detected" >&2
  exit 1
fi
if grep -Fq "Failed to execute /init" "$LOG"; then
  echo "kernel attempted a pathname init fallback" >&2
  exit 1
fi
if [ "${LUNA_REAL_INIT:-0}" = "1" ]; then
  echo "Real luna-init acceptance passed: direct PID1 -> LUNA-SYS/LUNA-DATA -> logical root -> runtime"
else
  echo "Canonical LUNA-SYS fixture acceptance passed"
fi
