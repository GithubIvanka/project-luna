#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ACTION="${1:-logout}"
case "$ACTION" in logout|reboot|shutdown) ;; *) echo "usage: $0 logout|reboot|shutdown" >&2; exit 2 ;; esac
TEST_ROOT="${LUNA_CANONICAL_TEST_ROOT:-$REPO_ROOT/target/canonical-lifecycle-$ACTION}"
mkdir -p "$TEST_ROOT/logs"

# Patch only the small immutable System Image into a disposable COW layer.
# The accepted source disk and its DATA are never modified or rebuilt here.
IMG="${LUNA_CANONICAL_IMAGE:-${LUNA_CANONICAL_OUTPUT_ROOT:-$REPO_ROOT/dist}/luna-0.1.0.img}"
TEST_IMG="$TEST_ROOT/guest.qcow2"
LOG="$TEST_ROOT/logs/serial.log"
[ ! -e "$TEST_IMG" ] && [ ! -L "$TEST_IMG" ] || { echo "test overlay already exists: $TEST_IMG" >&2; exit 1; }
SOCKET_ROOT="$(mktemp -d /tmp/luna-lifecycle.XXXXXX)"
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
  rm -rf "$SOCKET_ROOT"
  [ "${LUNA_CANONICAL_KEEP_TEST_DISKS:-0}" = 1 ] || rm -f "$TEST_IMG"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
python3 "$REPO_ROOT/tools/canonical-luna-artifacts.py" patch-system-image "$IMG" \
    --overlay "$TEST_IMG" --scratch "$TEST_ROOT" --session-action "$ACTION" \
    > "$TEST_ROOT/logs/patch.json"
sync -f "$TEST_IMG"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$TEST_ROOT/vars.fd"
qemu-system-x86_64 -name "Luna $ACTION acceptance" -machine q35 -m 2G -smp 4 -accel kvm -cpu host \
  -vga none -device "${LUNA_QEMU_GPU:-virtio-vga-gl}" -nic user,model=virtio-net-pci \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,file="$TEST_ROOT/vars.fd" -drive format=qcow2,file="$TEST_IMG" \
  -monitor "unix:$SOCKET_ROOT/h.sock,server=on,wait=off" \
  -qmp "unix:$SOCKET_ROOT/q.sock,server=on,wait=off" \
  -serial "file:$LOG" -display "${LUNA_QEMU_DISPLAY:-sdl,gl=on}" -no-shutdown \
  >"$TEST_ROOT/logs/qemu.log" 2>&1 &
QEMU_PID=$!

python3 - "$ACTION" "$SOCKET_ROOT" "$LOG" "$TEST_ROOT/logs" <<'PY'
from pathlib import Path
import json
import socket
import sys
import time

action, socket_root, serial_path, evidence_root = sys.argv[1:]
serial = Path(serial_path)
evidence = Path(evidence_root)
events = []
deadline = time.monotonic() + 150
for name in ('h.sock', 'q.sock'):
    while not (Path(socket_root) / name).exists():
        if time.monotonic() > deadline:
            raise SystemExit('QEMU control socket did not appear')
        time.sleep(.1)
qmp = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
qmp.connect(str(Path(socket_root) / 'q.sock'))
qmp.settimeout(.2)
buffer = bytearray()

def receive():
    global buffer
    try:
        data = qmp.recv(65536)
    except socket.timeout:
        return []
    if not data:
        raise RuntimeError('QEMU control connection closed')
    buffer.extend(data)
    messages = []
    while b'\n' in buffer:
        line, _, remaining = buffer.partition(b'\n')
        buffer = bytearray(remaining)
        if line.strip():
            message = json.loads(line)
            messages.append(message)
            if 'event' in message:
                events.append(message)
                (evidence / 'qmp-events.json').write_text(json.dumps(events, indent=2))
    return messages

receive()
qmp.sendall(b'{"execute":"qmp_capabilities"}\n')
receive()

def login():
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as hmp:
        hmp.connect(str(Path(socket_root) / 'h.sock'))
        hmp.settimeout(.2)
        hmp.recv(4096)
        for key in ('l', 'u', 'n', 'a', 'ret'):
            hmp.sendall(f'sendkey {key}\n'.encode())
            time.sleep(.3)

login_count = 0
while time.monotonic() < deadline:
    receive()
    log = serial.read_text(errors='replace') if serial.exists() else ''
    if any(failure in log for failure in ('Kernel panic', 'session UI I/O failed', 'status 139', 'no allocator available for device', 'persistent data LOST')):
        raise SystemExit('guest failure, see serial.log')
    ready = log.count('luna-user-session: login UI keyboard/pointer input ready')
    if login_count == 0 and ready >= 1:
        login()
        login_count = 1
    if action == 'logout' and login_count == 1 and ready >= 2:
        assert 'graphical UserSession ended; returning to login UI' in log
        # A second authenticated desktop proves that the restored login works.
        login()
        login_count = 2
    if action == 'reboot' and login_count == 1 and ready >= 2:
        login()
        login_count = 2
    # Planned intent is flushed while the shell is alive. The request line can
    # be lost when PID 1 terminates the serial tail during a real power action.
    requested = f'LUNA-LIFECYCLE: planned {action} through Noctalia' in log
    if requested and action == 'logout' and login_count == 2:
        if log.count('Noctalia process alive after 20s') >= 2:
            assert log.count('login credentials accepted') >= 2
            assert log.count('graphical UserSession launched') >= 2
            assert log.count('Wayland display ready') >= 2
            assert 'LUNA-LIFECYCLE: persistent home and config verified' in log
            assert log.count('Luna: luna-init is running as PID 1') == 1
            break
    if requested and action == 'reboot':
        if login_count == 2 and log.count('Noctalia process alive after 20s') >= 2:
            assert 'Luna: filesystems synced; executing reboot' in log
            assert "Operation 'reboot' finished." in log
            assert log.count('boot success confirmed') >= 2
            assert 'LUNA-LIFECYCLE: persistent home and config verified' in log
            import re
            revisions = re.findall(r'persistent system state ready in \d+ ms \(revision (\d+)\)', log)
            assert len(revisions) == 2 and revisions[0] == revisions[1], revisions
            assert any(e.get('event') == 'RESET' and e.get('data', {}).get('guest') for e in events)
            break
    if requested and action == 'shutdown':
        if any(e.get('event') == 'SHUTDOWN' and e.get('data', {}).get('guest') for e in events):
            assert 'Luna: filesystems synced; executing poweroff' in log
            assert "Operation 'poweroff' finished." in log
            break
    time.sleep(.1)
else:
    raise SystemExit(f'{action} lifecycle did not complete, see {serial}')
print(f'Canonical Luna {action} lifecycle verification: PASS')
print(f'Serial log: {serial}')
PY
sha256sum -c "$IMG.sha256"
