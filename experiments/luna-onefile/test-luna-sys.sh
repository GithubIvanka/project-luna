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
DATA_CONFIG_STALE_TEST="${LUNA_DATA_CONFIG_STALE_TEST:-0}"
REAL_SYSTEM_RUNTIME_TEST="${LUNA_REAL_SYSTEM_RUNTIME:-0}"
MANAGER_GATE_TEST="${LUNA_MANAGER_GATE_TEST:-0}"
MANAGER_GATE_FAILURE="${LUNA_MANAGER_GATE_FAILURE:-}"
MANAGER_GATE_INTERACTIVE_TEST="${LUNA_MANAGER_GATE_INTERACTIVE_TEST:-0}"
NIRI_SESSION_TEST="${LUNA_NIRI_SESSION_TEST:-0}"
NOCTALIA_SESSION_TEST="${LUNA_NOCTALIA_SESSION_TEST:-0}"
NIRI_QEMU_DISPLAY="${LUNA_NIRI_QEMU_DISPLAY:-egl-headless}"
NIRI_PROVIDER_ROOT="${LUNA_ONEFILE_EXTRA_DATA_ROOT:-}"
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
if [ "$DATA_CONFIG_STALE_TEST" = "1" ] && { [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ] || [ "$DATA_EXTERNAL_TEST" = "1" ]; }; then
  echo "DATA stale-config test mode is mutually exclusive" >&2
  exit 1
fi
if [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  [ -n "$RECOVERY_ROOT_SOURCE" ] || { echo "missing Recovery DATA source root" >&2; exit 1; }
  [ -d "$RECOVERY_ROOT_SOURCE" ] || { echo "missing Recovery DATA source root: $RECOVERY_ROOT_SOURCE" >&2; exit 1; }
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
if [ "${LUNA_REAL_INIT:-0}" = "1" ]; then
  mkdir -p "$OUT/minimal-root/libs" "$OUT/minimal-root/config"
  for resource_class in fonts icons themes cursors sounds locales translations; do
    mkdir -p "$OUT/minimal-root/resources/$resource_class"
    : > "$OUT/minimal-root/resources/$resource_class/system-resource-marker"
  done
  : > "$OUT/minimal-root/libs/system-lib-marker"
  : > "$OUT/minimal-root/config/system-config-marker"
fi
mksquashfs "$OUT/minimal-root" "$OUT/system-root/images/luna-0.1.0.squashfs" -noappend -quiet
if [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
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
elif [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  printf '%s\n' '[data]' 'preferred_disk_guid = "11111111-1111-4111-8111-111111111111"' 'preferred_partition_guid = "99999999-9999-4999-8999-999999999999"' > "$OUT/system-root/config/luna-data.toml"
fi
mkfs.ext4 -q -F -L LUNA-SYS -d "$OUT/system-root" "$OUT/system.img" 192M

mkdir -p "$OUT/data-root/system" "$OUT/data-root/users/luna"
if [ -n "${LUNA_REAL_INIT:-}" ]; then
  mkdir -p "$OUT/data-root/system/libs" "$OUT/data-root/system/config"
  mkdir -p "$OUT/data-root/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
  mkdir -p "$OUT/data-root/system/apps/data-probe"
  : > "$OUT/data-root/system/libs/data-lib-marker"
  : > "$OUT/data-root/system/config/data-config-marker"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$OUT/data-root/system/apps/data-probe/data-probe"
  chmod 0755 "$OUT/data-root/system/apps/data-probe/data-probe"
  for resource_class in fonts icons themes cursors sounds locales translations; do
    : > "$OUT/data-root/system/resources/$resource_class/data-resource-marker"
  done
fi
if [ "$NIRI_SESSION_TEST" = "1" ]; then
  [ -d "$NIRI_PROVIDER_ROOT/system" ] || { echo "missing staged Niri DATA provider: $NIRI_PROVIDER_ROOT" >&2; exit 1; }
  cp -a "$NIRI_PROVIDER_ROOT/system/." "$OUT/data-root/system/"
  if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
    DATA_SIZE=1024M
  else
    DATA_SIZE=768M
  fi
else
  DATA_SIZE=128M
fi

if [ "$RECOVERY_TEST" != "1" ]; then
  truncate -s "$DATA_SIZE" "$OUT/data.img"
  mkfs.btrfs -q -f -L LUNA-DATA --rootdir "$OUT/data-root" "$OUT/data.img"
  if [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
    cp "$OUT/data.img" "$OUT/data2.img"
  fi
fi

DISK_SIZE=448M
if [ "$NIRI_SESSION_TEST" = "1" ]; then
  if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
    DISK_SIZE=1344M
  else
    DISK_SIZE=1088M
  fi
elif [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_EXTERNAL_TEST" = "1" ]; then
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
elif [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  sgdisk --disk-guid=11111111-1111-4111-8111-111111111111 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+128M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
  sgdisk --partition-guid=1:22222222-2222-4222-8222-222222222222 --partition-guid=2:33333333-3333-4333-8333-333333333333 --partition-guid=3:44444444-4444-4444-8444-444444444444 "$OUT/disk.img" >/dev/null
elif [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_TEST" = "1" ]; then
  sgdisk --disk-guid=11111111-1111-4111-8111-111111111111 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+128M -t 3:8300 -c 3:LUNA-DATA -n 4:788480:+128M -t 4:8300 -c 4:LUNA-DATA "$OUT/disk.img" >/dev/null
  sgdisk --partition-guid=1:22222222-2222-4222-8222-222222222222 --partition-guid=2:33333333-3333-4333-8333-333333333333 --partition-guid=3:44444444-4444-4444-8444-444444444444 --partition-guid=4:55555555-5555-4555-8555-555555555555 "$OUT/disk.img" >/dev/null
else
  if [ "$NIRI_SESSION_TEST" = "1" ]; then
    if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
      sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D --partition-guid=3:7A6D5A7A-0003-4C55-4E41-444154412020 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+1024M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
    else
      sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D --partition-guid=3:7A6D5A7A-0003-4C55-4E41-444154412020 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+768M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
    fi
  else
    sgdisk --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D --partition-guid=3:7A6D5A7A-0003-4C55-4E41-444154412020 -n 1:2048:+64M -t 1:ef00 -c 1:EFI -n 2:133120:+192M -t 2:8300 -c 2:LUNA-SYS -n 3:526336:+128M -t 3:8300 -c 3:LUNA-DATA "$OUT/disk.img" >/dev/null
  fi
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
QEMU_MONITOR_ARGS=()
QEMU_MEMORY=512M
QEMU_VIDEO_ARGS=(-vga std)
QEMU_DISPLAY_ARGS=(-display none)
if [ "$NIRI_SESSION_TEST" = "1" ]; then
  QEMU_MEMORY=2G
  # Niri rejects software EGL renderers. Use VirGL and a host EGL display;
  # select egl-headless by default or another GL-capable QEMU backend for
  # hosts where egl-headless cannot access a DRM render node.
  QEMU_VIDEO_ARGS=(-vga none -device virtio-gpu-gl-pci)
  QEMU_DISPLAY_ARGS=(-display "$NIRI_QEMU_DISPLAY")
fi
QEMU_MONITOR_SOCKET="/tmp/luna-hmp-$$.sock"
INPUT_INJECTOR_PID=""
if [ "$MANAGER_GATE_INTERACTIVE_TEST" = "1" ]; then
  QEMU_MONITOR_ARGS=(-monitor "unix:$QEMU_MONITOR_SOCKET,server=on,wait=off")
  python3 - "$QEMU_MONITOR_SOCKET" "$LOG" <<'PY_INPUT' &
from pathlib import Path
import os
import socket
import sys
import time

monitor_path = sys.argv[1]
serial_log = Path(sys.argv[2])
ready_timeout = 20.0 if os.environ.get("LUNA_NIRI_SESSION_TEST") == "1" else 10.0
deadline = time.monotonic() + ready_timeout
ready_marker = "luna-user-session: login UI keyboard/pointer input ready"
while time.monotonic() < deadline:
    if serial_log.exists() and ready_marker in serial_log.read_text(errors="replace"):
        break
    time.sleep(0.1)
else:
    print("keyboard injector: UserSession input readiness was not observed", file=sys.stderr)
    raise SystemExit(1)

connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
while time.monotonic() < deadline:
    try:
        connection.connect(monitor_path)
        break
    except (FileNotFoundError, ConnectionRefusedError):
        time.sleep(0.1)
else:
    print("keyboard injector: QEMU monitor did not become available", file=sys.stderr)
    raise SystemExit(1)

with connection:
    # The fixture has one eligible account. Send normal key presses for
    # the temporary test password; no authentication API is bypassed.
    for key in ("t", "e", "s", "t", "1", "2", "3", "ret"):
        connection.sendall(f"sendkey {key}\n".encode())
        time.sleep(0.22)
print("keyboard injector: sent test password through QEMU HMP")
PY_INPUT
  INPUT_INJECTOR_PID=$!
fi
set +e
timeout "${LUNA_ONEFILE_QEMU_TIMEOUT:-15s}" qemu-system-x86_64 -machine q35 -m "$QEMU_MEMORY" "${QEMU_VIDEO_ARGS[@]}" "${QEMU_DISPLAY_ARGS[@]}" -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" -drive format=raw,file="$OUT/disk.img" "${QEMU_EXTRA_DRIVES[@]}" "${QEMU_MONITOR_ARGS[@]}" -serial "file:$LOG" -no-reboot -no-shutdown >"$OUT/qemu.log" 2>&1
status=$?
set -e
if [ -n "$INPUT_INJECTOR_PID" ]; then
  wait "$INPUT_INJECTOR_PID" || true
fi
if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
  echo "QEMU failed with status $status" >&2
  exit "$status"
fi

if [ "${LUNA_DIRECT_INIT:-0}" = "1" ]; then
  if [ "${LUNA_RECOVERY_TEST:-0}" != "1" ] && [ "${LUNA_DATA_AMBIGUOUS_TEST:-0}" != "1" ] && [ "${LUNA_DATA_CONFIG_STALE_TEST:-0}" != "1" ]; then
    grep -Fq "luna_boot: Luna: boot availability handoff=1 init=1" "$LOG"
    grep -Fq "luna_exec: Luna: preparing memory-resident luna-init for direct PID 1" "$LOG"
    grep -Fq "luna_exec: Luna: executing memory-resident luna-init as PID 1" "$LOG"
  fi
else
  grep -Fq "Run /init as init process" "$LOG"
fi

if [ "${LUNA_RECOVERY_TEST:-0}" = "1" ] || [ "${LUNA_DATA_AMBIGUOUS_TEST:-0}" = "1" ] || [ "${LUNA_DATA_CONFIG_STALE_TEST:-0}" = "1" ]; then
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

if [ "$MANAGER_GATE_TEST" = "1" ]; then
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "luna-system-runtime: System Image 0.1.0, init 0.1.0, kernel 0.1.0" "$LOG"

  case "$MANAGER_GATE_FAILURE" in
    "")
      grep -Fq "luna-system-runtime: started required system service /usr/bin/luna-device-manager" "$LOG"
      grep -Fq "luna-system-runtime: started required system service /usr/bin/luna-volume-manager" "$LOG"
      grep -Fq "luna-device-manager: native kernel uevent monitor ready" "$LOG"
      grep -Fq "luna-device-manager: ready; discovered" "$LOG"
      grep -Fq "luna-volume-manager: ready; auto-attach policy enabled=true" "$LOG"
      grep -Fq "luna-system-runtime: device and volume managers ready (detected=" "$LOG"
      grep -Fq "luna-system-runtime: boot success confirmed" "$LOG"
      ! grep -Fq "luna-system-runtime: native system service startup failed:" "$LOG"

      device_ready_line=$(grep -nF "luna-device-manager: ready; discovered" "$LOG" | head -n1 | cut -d: -f1)
      volume_ready_line=$(grep -nF "luna-volume-manager: ready; auto-attach policy enabled=true" "$LOG" | head -n1 | cut -d: -f1)
      gate_line=$(grep -nF "luna-system-runtime: device and volume managers ready (" "$LOG" | head -n1 | cut -d: -f1)
      success_line=$(grep -nF "luna-system-runtime: boot success confirmed" "$LOG" | head -n1 | cut -d: -f1)
      [ "$device_ready_line" -lt "$gate_line" ]
      [ "$volume_ready_line" -lt "$gate_line" ]
      [ "$gate_line" -lt "$success_line" ]

      # The same direct-init fixture must expose an actual KMS surface and draw
      # the native login frame, not merely confirm the earlier boot gate.
      grep -Fq "luna-user-session: login UI graphics device /dev/dri/card0" "$LOG"
      grep -Fq "luna-user-session: login UI acquired DRM master" "$LOG"
      grep -Fq "luna-user-session: login UI KMS target connector=" "$LOG"
      grep -Fq "luna-user-session: login UI initial frame presented" "$LOG"
      grep -Fq "luna-user-session: login UI first frame drawn" "$LOG"
      grep -Fq "luna-user-session: login UI keyboard/pointer input ready" "$LOG"
      ! grep -Fq "session UI I/O failed: No such file or directory" "$LOG"

      success_line=$(grep -nF "luna-system-runtime: boot success confirmed" "$LOG" | head -n1 | cut -d: -f1)
      drm_line=$(grep -nF "luna-user-session: login UI acquired DRM master" "$LOG" | head -n1 | cut -d: -f1)
      frame_line=$(grep -nF "luna-user-session: login UI first frame drawn" "$LOG" | head -n1 | cut -d: -f1)
      input_line=$(grep -nF "luna-user-session: login UI keyboard/pointer input ready" "$LOG" | head -n1 | cut -d: -f1)
      [ "$success_line" -lt "$drm_line" ]
      [ "$drm_line" -lt "$frame_line" ]
      [ "$frame_line" -lt "$input_line" ]

      if [ "$MANAGER_GATE_INTERACTIVE_TEST" = "1" ]; then
        grep -Fq "luna-user-session: login credentials accepted" "$LOG"
        grep -Fq "luna-system-runtime: graphical UserSession launched /usr/bin/luna-graphical-session" "$LOG"
        if [ "$NIRI_SESSION_TEST" = "1" ]; then
          grep -Fq "luna-device-manager: udev compatibility backend ready" "$LOG"
          grep -Fq "luna-system-runtime: started optional seat compatibility provider /usr/bin/seatd" "$LOG"
          grep -Fq "LUNA-NIRI: LAUNCHING /usr/bin/niri --session" "$LOG"
          grep -Fq "LUNA-NIRI: WAYLAND_READY " "$LOG"
          grep -Fq "new connector: Virtual-1" "$LOG"
          grep -Fq "connecting connector: Virtual-1" "$LOG"
          grep -Fq 'picking mode: Mode { name: "1280x800"' "$LOG"
          if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
            grep -Fq "LUNA-NOCTALIA: LAUNCHING /usr/bin/noctalia" "$LOG"
            grep -Fq "LUNA-NOCTALIA: PROCESS_ALIVE" "$LOG"
            if grep -Eq "LUNA-NOCTALIA: PROCESS_(NOT_FOUND|EXITED)" "$LOG"; then
              echo "Noctalia did not stay alive through the observation window" >&2
              exit 1
            fi
            grep -Fq "connected to session bus" "$LOG"
            grep -Fq "connected compositor=yes shm=yes layer-shell=yes" "$LOG"
            grep -Fq "initialized EGL 1.5" "$LOG"
            grep -Fq 'OpenGL ES vendor="Mesa" renderer="virgl' "$LOG"
            grep -Fq "path=/usr/share/noctalia/assets/noctalia-wallpaper.png" "$LOG"
            grep -Fq 'creating #0 "default" on Virtual-1' "$LOG"
          fi
          if grep -Fq "LUNA-NIRI: WAYLAND_NOT_READY" "$LOG"; then
            echo "Niri did not create its Wayland socket" >&2
            exit 1
          fi
          if grep -Fq "no allocator available for device" "$LOG" ||
             grep -Fq "software EGL renderers are skipped" "$LOG"; then
            echo "Niri started without a working DRM/GBM allocator" >&2
            exit 1
          fi
          if grep -Fq "graphical desktop exited" "$LOG" ||
             grep -Eq "thread 'main'.*panicked|^Aborted$" "$LOG"; then
            echo "Niri graphical session exited or panicked during acceptance" >&2
            exit 1
          fi
        else
          grep -Fq "LUNA-NATIVE-SESSION: READY" "$LOG"
        fi
      fi
      ;;
    missing-volume-manager)
      grep -Fq "luna-system-runtime: native system service startup failed:" "$LOG"
      grep -Fq "failed to spawn child process: No such file or directory" "$LOG"
      ! grep -Fq "luna-system-runtime: boot success confirmed" "$LOG"
      ! grep -Fq "luna-system-runtime: graphical UserSession launched" "$LOG"
      ;;
    no-available-volumes)
      grep -Fq "luna-volume-manager did not make any volumes available (detected=1, available=0)" "$LOG"
      ! grep -Fq "luna-system-runtime: boot success confirmed" "$LOG"
      ! grep -Fq "luna-system-runtime: graphical UserSession launched" "$LOG"
      ;;
    *)
      echo "unknown manager-gate failure mode: $MANAGER_GATE_FAILURE" >&2
      exit 2
      ;;
  esac
elif [ "$REAL_SYSTEM_RUNTIME_TEST" = "1" ]; then
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "luna-system-runtime: System Image 0.1.0, init 0.1.0, kernel 0.1.0" "$LOG"
  grep -Fq "luna-system-runtime: boot success confirmed" "$LOG"
  grep -Fq "luna-system-runtime: graphical UserSession launched /usr/bin/luna-graphical-session" "$LOG"
  grep -Fq "LUNA-NATIVE-SESSION: READY" "$LOG"
  ! grep -Fq "luna-system-runtime: OpenRC starting" "$LOG"
elif [ "$LUNA_REAL_INIT" = "1" ]; then
  grep -Fq "Luna: luna-init is running as PID 1" "$LOG"
  grep -Fq "Luna: logical root ready; starting luna-system-runtime" "$LOG"
  grep -Fq "LUNA-LOGICAL-ROOT-CONTRACT: READY" "$LOG"
  grep -Fq "LUNA-SYSTEM-RUNTIME: READY" "$LOG"
else
  grep -Fq "LUNA-INIT: PID 1" "$LOG"
  grep -Fq "LUNA-READY" "$LOG"
fi

if [ "$MANAGER_GATE_TEST" = "1" ] && [ -n "$MANAGER_GATE_FAILURE" ]; then
  # PID 1 exit can trigger the existing luna-init failure path and a downstream
  # kernel panic after the runtime has logged the deliberate gate rejection.
  if grep -Eq "No working init|Failed to execute" "$LOG"; then
    echo "fatal boot error unrelated to the expected manager-gate rejection" >&2
    exit 1
  fi
  if grep -Eq "Kernel panic|general protection" "$LOG" && ! grep -Fq "luna-system-runtime: native system service startup failed:" "$LOG"; then
    echo "unexpected kernel failure without an expected manager-gate rejection" >&2
    exit 1
  fi
elif grep -Eq "Kernel panic|general protection|No working init|Failed to execute" "$LOG"; then
  echo "fatal boot error detected" >&2
  exit 1
fi
if grep -Fq "Failed to execute /init" "$LOG"; then
  echo "kernel attempted a pathname init fallback" >&2
  exit 1
fi
if [ "$MANAGER_GATE_TEST" = "1" ]; then
  if [ -z "$MANAGER_GATE_FAILURE" ]; then
    echo "Luna-native manager boot gate acceptance passed: direct PID1 -> logical root -> Device Manager -> Volume Manager -> boot success"
  else
    echo "Luna-native manager boot gate negative acceptance passed: failure=$MANAGER_GATE_FAILURE prevented boot success"
  fi
elif [ "${LUNA_REAL_INIT:-0}" = "1" ]; then
  echo "Real luna-init acceptance passed: direct PID1 -> LUNA-SYS/LUNA-DATA -> logical root -> runtime"
else
  echo "Canonical LUNA-SYS fixture acceptance passed"
fi
