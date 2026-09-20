#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${LUNA_PC_IMAGE:-$REPO_ROOT/dist/luna-pc.img}"
OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
OUT="$REPO_ROOT/dist/logs/production-ovmf"
SERIAL_LOG="$OUT-serial.log"
QEMU_LOG="$OUT-qemu.log"
VARS_COPY="$REPO_ROOT/dist/.build/production-OVMF-VARS.fd"

command -v qemu-system-x86_64 >/dev/null || { echo "qemu-system-x86_64 is required" >&2; exit 1; }
command -v sgdisk >/dev/null || { echo "sgdisk is required" >&2; exit 1; }
command -v blkid >/dev/null || { echo "blkid is required" >&2; exit 1; }
for input in "$IMAGE" "$OVMF_CODE" "$OVMF_VARS"; do
    [ -f "$input" ] || { echo "missing input: $input" >&2; exit 1; }
done

mkdir -p "$(dirname "$VARS_COPY")"
cp "$OVMF_VARS" "$VARS_COPY"
rm -f "$SERIAL_LOG" "$QEMU_LOG"

echo "--- GPT ---"
sgdisk -p "$IMAGE"
SYS_START="$(sgdisk -i 2 "$IMAGE" | awk '/First sector:/ {print $3; exit}')"
DATA_START="$(sgdisk -i 3 "$IMAGE" | awk '/First sector:/ {print $3; exit}')"
OFF2=$((SYS_START * 512))
OFF3=$((DATA_START * 512))
SYS_TYPE="$(blkid -p -O "$OFF2" -s TYPE -o value "$IMAGE")"
DATA_TYPE="$(blkid -p -O "$OFF3" -s TYPE -o value "$IMAGE")"
[ "$SYS_TYPE" = ext4 ] || { echo "LUNA-SYS is not ext4: $SYS_TYPE" >&2; exit 1; }
[ "$DATA_TYPE" = btrfs ] || { echo "LUNA-DATA is not btrfs: $DATA_TYPE" >&2; exit 1; }

echo "--- OVMF production boot ---"
set +e
timeout 30s qemu-system-x86_64 \
  -machine q35 \
  -m 2G \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$VARS_COPY" \
  -drive format=raw,file="$IMAGE" \
  -serial "file:$SERIAL_LOG" \
  -display none \
  -no-reboot \
  -no-shutdown \
  -D "$QEMU_LOG"
rc=$?
set -e
[ "$rc" -eq 124 ] || { echo "production OVMF exited before timeout: $rc" >&2; exit "$rc"; }

grep -q 'BTRFS: device label LUNA-DATA' "$SERIAL_LOG" \
  || { echo "LUNA-DATA btrfs scan marker missing" >&2; exit 1; }
grep -q 'luna-device-manager: ready' "$SERIAL_LOG" \
  || { echo "device-manager ready marker missing" >&2; exit 1; }
grep -q 'luna-system-runtime: started system service /usr/bin/dbus-daemon' "$SERIAL_LOG" \
  || { echo "D-Bus start marker missing" >&2; exit 1; }
! grep -q 'luna-system-runtime: system service /usr/bin/dbus-daemon did not become ready' "$SERIAL_LOG" \
  || { echo "system D-Bus did not become ready" >&2; exit 1; }
grep -q 'luna-system-runtime: boot success confirmed' "$SERIAL_LOG" \
  || { echo "boot success marker missing" >&2; exit 1; }
grep -q 'luna-system-runtime: graphical UserSession launched /usr/bin/niri --session' "$SERIAL_LOG" \
  || { echo "real Niri launch marker missing" >&2; exit 1; }
! grep -Eiq 'greetd|greeter' "$SERIAL_LOG" \
  || { echo "legacy greetd/greeter path appeared" >&2; exit 1; }
! grep -Eq 'Kernel panic|panicked at' "$SERIAL_LOG" \
  || { echo "kernel/userspace panic detected" >&2; exit 1; }

echo "Production OVMF verification: PASS"
