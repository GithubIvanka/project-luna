#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_ROOT="${LUNA_CANONICAL_OUTPUT_ROOT:-$REPO_ROOT/dist}"
IMAGE="${LUNA_CANONICAL_IMAGE:-$OUTPUT_ROOT/luna-0.1.0.img}"
TEST_ROOT="${LUNA_CANONICAL_TEST_ROOT:-$OUTPUT_ROOT/.build/boot-display}"
mkdir -p "$TEST_ROOT"

# Check firmware display continuity separately from accelerated GL scanout:
# QMP screendump cannot read that scanout after the guest DRM driver takes over.
python3 - "$IMAGE" "$TEST_ROOT" <<'PY'
from pathlib import Path
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

image, root = (Path(p).resolve() for p in sys.argv[1:])
shutil.copy('/usr/share/OVMF/OVMF_VARS_4M.fd', root / 'vars.fd')
rows = []
with tempfile.TemporaryDirectory(prefix='luna-display-', dir='/tmp') as sockets:
    qmp_path = Path(sockets) / 'q.sock'
    with (root / 'qemu.log').open('w') as qemu_log:
        process = subprocess.Popen([
            'qemu-system-x86_64', '-machine', 'q35', '-m', '2G', '-smp', '4',
            '-accel', 'kvm', '-cpu', 'host', '-vga', 'none',
            '-device', os.environ.get('LUNA_QEMU_GPU', 'virtio-vga-gl'),
            '-drive', 'if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd',
            '-drive', f'if=pflash,format=raw,file={root}/vars.fd',
            '-drive', f'format=raw,snapshot=on,file={image}',
            '-qmp', f'unix:{qmp_path},server=on,wait=off',
            '-serial', f'file:{root}/serial.log',
            '-display', os.environ.get('LUNA_QEMU_DISPLAY', 'sdl,gl=on'),
            '-no-reboot', '-no-shutdown',
        ], stdout=subprocess.DEVNULL, stderr=qemu_log)
        try:
            deadline = time.monotonic() + 45
            while not qmp_path.exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('QEMU did not start; see qemu.log')
                time.sleep(.05)
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as qmp:
                qmp.settimeout(5)
                qmp.connect(str(qmp_path))
                with qmp.makefile('rwb', buffering=0) as stream:
                    json.loads(stream.readline())
                    def command(payload):
                        stream.write((json.dumps(payload) + '\n').encode())
                        while True:
                            response = json.loads(stream.readline())
                            if 'return' in response or 'error' in response:
                                return response
                    command({'execute': 'qmp_capabilities'})
                    start = time.monotonic()
                    while time.monotonic() < deadline:
                        frame = root / 'firmware.ppm'
                        frame.unlink(missing_ok=True)
                        response = command({'execute': 'screendump', 'arguments': {'filename': str(frame)}})
                        log = (root / 'serial.log').read_text(errors='replace')
                        row = dict(elapsed=round(time.monotonic() - start, 3),
                                   surface=frame.exists(), error=response.get('error'),
                                   kernel_started='Linux version' in log,
                                   login_ready='login UI keyboard/pointer input ready' in log)
                        rows.append(row)
                        if 'Kernel panic' in log or 'session UI I/O failed' in log:
                            raise RuntimeError('guest failed; see serial.log')
                        if row['login_ready']:
                            break
                        time.sleep(.1)
                    else:
                        raise RuntimeError('native login did not become ready')
            firmware = [row for row in rows if not row['kernel_started']]
            if not firmware or not all(row['surface'] for row in firmware):
                raise RuntimeError('QEMU lost its firmware display surface; see frames.json')
            print(f'Canonical Luna firmware display verification: PASS ({len(firmware)} firmware samples)')
        finally:
            try:
                (root / 'frames.json').write_text(json.dumps(rows, indent=2))
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
PY
sha256sum -c "$IMAGE.sha256"
