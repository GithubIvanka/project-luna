#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_ROOT="${LUNA_CANONICAL_OUTPUT_ROOT:-$REPO_ROOT/dist}"
TEST_ROOT="${LUNA_CANONICAL_TEST_ROOT:-$OUTPUT_ROOT/.build}"
IMG="${LUNA_CANONICAL_IMAGE:-$OUTPUT_ROOT/luna-0.1.0.img}"
TEST_IMG="$TEST_ROOT/canonical-login-test.qcow2"
VARS="$TEST_ROOT/canonical-login-test-vars.fd"
# Keep the Unix-domain socket path short enough for sockaddr_un.sun_path.
MONITOR="${LUNA_CANONICAL_MONITOR_PATH:-$TEST_ROOT/m.sock}"
LOG="$TEST_ROOT/logs/canonical-login-test-serial.log"
QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-sdl,gl=on}"
OBSERVATION_SECONDS="${LUNA_CANONICAL_SESSION_SECONDS:-60}"
LOGIN_TIMEOUT_SECONDS="${LUNA_CANONICAL_LOGIN_TIMEOUT_SECONDS:-20}"
SCREENSHOT="$TEST_ROOT/logs/canonical-login-test.ppm"
QEMU_LOG="$TEST_ROOT/logs/canonical-login-test-qemu.log"

assert_log_absent() {
  if grep -Fq "$1" "$LOG"; then
    echo "canonical session failed: $1 (see $LOG)" >&2
    exit 1
  fi
}

mkdir -p "$TEST_ROOT/logs"
[ -f "$IMG" ] || { echo "canonical image missing: $IMG" >&2; exit 1; }
[ ! -e "$TEST_IMG" ] && [ ! -L "$TEST_IMG" ] || { echo "test overlay already exists: $TEST_IMG" >&2; exit 1; }
QEMU_PID=""
cleanup() {
  if [ -n "$QEMU_PID" ]; then
    kill "$QEMU_PID" 2>/dev/null || true
    for _ in $(seq 1 30); do
      kill -0 "$QEMU_PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -KILL "$QEMU_PID" 2>/dev/null || true
    wait "$QEMU_PID" 2>/dev/null || true
  fi
  rm -f "$MONITOR"
  [ "${LUNA_CANONICAL_KEEP_TEST_DISKS:-0}" = 1 ] || rm -f "$TEST_IMG"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
python3 "$REPO_ROOT/tools/canonical-luna-artifacts.py" overlay "$IMG" "$TEST_IMG"
sync -f "$TEST_IMG"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS"
rm -f "$MONITOR" "$LOG" "$SCREENSHOT"

QEMU_LAUNCH=(env)
if [ "${LUNA_CANONICAL_TRACE_SYNC:-0}" = 1 ]; then
  QEMU_LAUNCH+=(strace --seccomp-bpf -f -ttt -T -e trace=fsync,fdatasync -o "$TEST_ROOT/logs/host-sync.trace")
fi
"${QEMU_LAUNCH[@]}" qemu-system-x86_64 -name "Luna canonical login test" -machine q35 -m 2G -smp 4 -accel kvm -cpu host \
  -vga none -device "${LUNA_QEMU_GPU:-virtio-vga-gl}" -nic user,model=virtio-net-pci \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file="$VARS" -drive format=qcow2,file="$TEST_IMG" \
  -monitor "unix:$MONITOR,server=on,wait=off" -serial "file:$LOG" -display "$QEMU_DISPLAY" -no-reboot -no-shutdown \
  >"$QEMU_LOG" 2>&1 &
QEMU_PID=$!

for _ in $(seq 1 100); do
  [ -S "$MONITOR" ] && break
  sleep 0.2
done
[ -S "$MONITOR" ] || { echo "QEMU monitor socket did not appear" >&2; exit 1; }

for _ in $(seq 1 "$((LOGIN_TIMEOUT_SECONDS * 4))"); do
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

sleep "$OBSERVATION_SECONDS"

grep -nE "login credentials accepted|authentication accepted|graphical UserSession launched|graphical desktop exited|session child start failed|session UI I/O failed|Kernel panic|niri" "$LOG" | tail -160

for failure in \
  "session child start failed" "session UI I/O failed" "graphical desktop exited" \
  "graphical UserSession ended; returning to login UI" "Niri exited with status" \
  "no allocator available for device" "software EGL renderers are skipped" \
  "virtio_gpu: driver missing" "BadKeymap" "Kernel panic" \
  "Error loading shared library" "Error relocating" \
  "Noctalia exited with status" \
  "Wayland display not observed" "Noctalia process not observed" "Noctalia process exited"; do
  assert_log_absent "$failure"
done

grep -q "luna-user-session: login credentials accepted" "$LOG"
grep -q "luna-system-runtime: graphical UserSession launched /usr/bin/luna-graphical-session" "$LOG"
grep -q "luna-graphical-session: starting user D-Bus" "$LOG"
grep -q "luna-graphical-session: user D-Bus ready" "$LOG"
grep -q "luna-graphical-session: PipeWire/WirePlumber started" "$LOG"
grep -q "luna-graphical-session: starting Niri" "$LOG"
grep -q "luna-graphical-session: Wayland display ready" "$LOG"
grep -q "luna-graphical-session: Noctalia process ready" "$LOG"
grep -Fq "luna-graphical-session: Noctalia process alive after 20s" "$LOG"
grep -Fq "new connector: Virtual-1" "$LOG"
grep -Fq "connecting connector: Virtual-1" "$LOG"
grep -Fq 'picking mode: Mode { name: "1280x800"' "$LOG"
grep -Fq "connected to session bus" "$LOG"
grep -Fq "connected compositor=yes shm=yes layer-shell=yes" "$LOG"
grep -Fq "initialized EGL 1.5" "$LOG"
grep -Fq 'OpenGL ES vendor="Mesa" renderer="virgl' "$LOG"
grep -Fq "path=/resources/images/wallpaper-night-sky.png" "$LOG"
grep -Fq 'creating #0 "default" on Virtual-1' "$LOG"

# HMP can capture CPU surfaces; a GL scanout may have no such surface.
python3 - "$MONITOR" "$SCREENSHOT" <<'PY'
import json
from pathlib import Path
import re
import socket
import sys
import time

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
    sock.connect(sys.argv[1])
    sock.settimeout(0.5)
    sock.recv(4096)
    sock.sendall(("screendump " + json.dumps(sys.argv[2]) + "\n").encode())
    # HMP echoes every keystroke with readline escapes. Drain it continuously;
    # otherwise the socket can fill before QEMU handles the full command.
    response = bytearray()
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            part = sock.recv(65536)
        except socket.timeout:
            continue
        if not part:
            break
        response.extend(part)
        # Readline repeats the prompt while echoing the command. Wait for an
        # actual result rather than mistaking that echo for completion.
        if Path(sys.argv[2]).exists() or b"Error: no surface" in response:
            break
    if not Path(sys.argv[2]).exists():
        diagnostic = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", response.decode(errors="replace"))
        if "Error: no surface" in diagnostic:
            print("HMP screenshot unavailable for this GL scanout (no CPU surface).")
        else:
            print(diagnostic[-1500:], file=sys.stderr)
            raise SystemExit("QEMU screen capture failed")
PY
sha256sum -c "$IMG.sha256"

echo "Canonical Luna login/session verification: PASS"
echo "Serial log: $LOG"
if [ -s "$SCREENSHOT" ]; then
  echo "Guest screenshot: $SCREENSHOT"
fi
