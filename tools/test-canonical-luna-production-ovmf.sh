#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_ROOT="${LUNA_CANONICAL_OUTPUT_ROOT:-$REPO_ROOT/dist}"
TEST_ROOT="${LUNA_CANONICAL_TEST_ROOT:-$OUTPUT_ROOT/.build}"
IMG="${LUNA_CANONICAL_IMAGE:-$OUTPUT_ROOT/luna-0.1.0.img}"
VARS="$TEST_ROOT/canonical-production-test-vars.fd"
LOG="$TEST_ROOT/logs/canonical-production-test-serial.log"
QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-sdl,gl=on}"

assert_log_absent() {
  if grep -Fq "$1" "$LOG"; then
    echo "canonical production boot failed: $1 (see $LOG)" >&2
    exit 1
  fi
}

mkdir -p "$TEST_ROOT/logs"
[ -f "$IMG" ] || { echo "canonical image missing: $IMG" >&2; exit 1; }
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS"
rm -f "$LOG"

set +e
timeout -k 5s 20s qemu-system-x86_64 -machine q35 -m 2G -smp 4 -accel kvm -cpu host   -vga none -device "${LUNA_QEMU_GPU:-virtio-vga-gl}"   -nic user,model=virtio-net-pci   -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd   -drive if=pflash,format=raw,file="$VARS"   -drive format=raw,snapshot=on,file="$IMG"   -serial "file:$LOG" -display "$QEMU_DISPLAY" -no-reboot -no-shutdown
rc=$?
set -e

[ "$rc" -eq 124 ] || { echo "unexpected QEMU exit: $rc" >&2; exit 1; }

grep -q "luna_boot: Luna: handoff v1 accepted" "$LOG"
grep -q "luna_exec: Luna: luna-init exec transition accepted" "$LOG"
grep -q "Luna: luna-init is running as PID 1" "$LOG"
grep -q "Luna: logical root ready; starting luna-system-runtime" "$LOG"
grep -q "luna-system-runtime: boot success confirmed" "$LOG"
grep -q "luna-system-runtime: starting optional compatibility runlevel sysinit" "$LOG"
grep -q "luna-system-runtime: starting optional compatibility runlevel boot" "$LOG"
grep -q "luna-system-runtime: starting optional compatibility runlevel default" "$LOG"
grep -q "luna-system-runtime: device and volume managers ready (detected=" "$LOG"
grep -q "auto_attach=true)" "$LOG"
READY_LINE="$(grep -n -m1 'luna-system-runtime: device and volume managers ready' "$LOG" | cut -d: -f1)"
SUCCESS_LINE="$(grep -n -m1 'luna-system-runtime: boot success confirmed' "$LOG" | cut -d: -f1)"
[ -n "$READY_LINE" ] && [ -n "$SUCCESS_LINE" ] && [ "$READY_LINE" -lt "$SUCCESS_LINE" ] || {
  echo "boot success was confirmed before Device Manager and Volume Manager became ready" >&2
  exit 1
}
python3 "$REPO_ROOT/tools/check-luna-service-starts.py" "$LOG"
assert_log_absent "Starting Luna Device Manager"
assert_log_absent "Starting Luna Volume Manager"
grep -Fq "New seat seat0." "$LOG"
grep -q "luna-user-session: login renderer font loaded from /resources/fonts/AdwaitaSans-Regular.ttf" "$LOG"
grep -q "luna-user-session: login renderer background loaded from /resources/images/wallpaper-night-sky.png" "$LOG"
grep -q "luna-user-session: login UI graphics device /dev/dri/card0" "$LOG"
grep -q "luna-user-session: login UI acquired DRM master" "$LOG"
grep -q "luna-user-session: login UI KMS target connector=" "$LOG"
grep -q "luna-user-session: login UI keyboard/pointer input ready" "$LOG"
grep -q "luna-user-session: login UI graphics device /dev/dri/card0" "$LOG"
grep -q "luna-user-session: login UI acquired DRM master" "$LOG"
grep -q "luna-system-runtime: boot success confirmed" "$LOG"

assert_log_absent "Kernel panic"
assert_log_absent "session UI I/O failed"
assert_log_absent "specified group 'video' unknown"
assert_log_absent "specified group 'audio' unknown"
assert_log_absent "specified group 'disk' unknown"
assert_log_absent "checkpath: owner 'root:uucp' not found"
assert_log_absent "failed to execute '/usr/bin/elogind-uaccess-command'"
assert_log_absent "failed to execute '/usr/libexec/elogind/elogind-uaccess-command'"

sha256sum -c "$IMG.sha256"

echo "Canonical Luna production OVMF verification: PASS"
echo "Serial log: $LOG"
