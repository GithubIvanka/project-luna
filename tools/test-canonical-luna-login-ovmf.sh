#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMG="$REPO_ROOT/dist/luna-0.1.0.img"
TEST_IMG="$REPO_ROOT/dist/.build/canonical-login-test.img"
VARS="$REPO_ROOT/dist/.build/canonical-login-test-vars.fd"
MONITOR="$REPO_ROOT/dist/.build/canonical-login-test.monitor"
LOG="$REPO_ROOT/dist/logs/canonical-login-test-serial.log"
QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-sdl,gl=on}"

cp --reflink=auto "$IMG" "$TEST_IMG"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS"
rm -f "$MONITOR" "$LOG"

qemu-system-x86_64 -name "Luna canonical login test" -machine q35 -m 2G -smp 4 -accel kvm -cpu host \
  -vga none -device virtio-gpu-gl-pci -nic user,model=virtio-net-pci \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file="$VARS" -drive format=raw,snapshot=on,file="$TEST_IMG" \
  -monitor "unix:$MONITOR,server=on,wait=off" -serial "file:$LOG" -display "$QEMU_DISPLAY" -no-reboot -no-shutdown \
  >/dev/null 2>&1 &
QEMU_PID=$!

cleanup() {
  kill "$QEMU_PID" 2>/dev/null || true
  wait "$QEMU_PID" 2>/dev/null || true
  rm -f "$MONITOR"
  # Keep the serial log for post-test diagnosis.
}
trap cleanup EXIT

for _ in $(seq 1 100); do
  [ -S "$MONITOR" ] && break
  sleep 0.2
done
[ -S "$MONITOR" ] || { echo "QEMU monitor socket did not appear" >&2; exit 1; }

for _ in $(seq 1 80); do
  grep -q "luna-user-session: login UI keyboard/pointer input ready" "$LOG" && break
  sleep 0.25
done
grep -q "luna-user-session: login UI keyboard/pointer input ready" "$LOG" || {
  echo "native login UI did not become ready" >&2
  exit 1
}

python3 - "$MONITOR" <<'PY'
import socket
import sys
import time

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(sys.argv[1])
time.sleep(0.2)
for command in ["sendkey l", "sendkey u", "sendkey n", "sendkey a", "sendkey ret"]:
    sock.sendall((command + "\n").encode())
    time.sleep(0.3)
sock.close()
PY

sleep 20

grep -nE "login credentials accepted|authentication accepted|graphical UserSession launched|graphical desktop exited|session child start failed|session UI I/O failed|Kernel panic|niri" "$LOG" | tail -160

grep -q "luna-user-session: login credentials accepted" "$LOG"
grep -q "luna-system-runtime: graphical UserSession launched /usr/bin/luna-graphical-session" "$LOG"
grep -q "luna-graphical-session: starting user D-Bus" "$LOG"
grep -q "luna-graphical-session: user D-Bus ready" "$LOG"
grep -q "luna-graphical-session: PipeWire/WirePlumber started" "$LOG"
grep -q "luna-graphical-session: starting Niri" "$LOG"
grep -q "luna-graphical-session: Wayland display ready" "$LOG"
grep -q "luna-graphical-session: Noctalia process ready" "$LOG"
! grep -q "session child start failed" "$LOG"
! grep -q "session UI I/O failed" "$LOG"
! grep -q "graphical desktop exited" "$LOG"
! grep -q "no allocator available for device" "$LOG"
! grep -q "software EGL renderers are skipped" "$LOG"
! grep -q "BadKeymap" "$LOG"
! grep -q "Kernel panic" "$LOG"

echo "Canonical Luna login/session verification: PASS"
