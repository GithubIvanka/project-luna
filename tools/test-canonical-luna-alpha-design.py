#!/usr/bin/env python3
"""Проверка реальных экранов Alpha в удаляемом QEMU-слое канонического образа."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import time

SPEC = importlib.util.spec_from_file_location('artifacts', Path(__file__).with_name('canonical-luna-artifacts.py'))
artifacts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(artifacts)

# Временная наблюдающая задача использует настоящий compositor и установленные
# приложения. Она не входит в образ и не изменяет контракт сессии/аутентификации.
SESSION = r'''(
    set -eu
    trap 'code=$?; echo "LUNA-ALPHA: observer exit $code"' EXIT
    for n in $(seq 1 160); do
        for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
            display="${socket##*/}"
            if [ -S "$socket" ] && [ -S "$XDG_RUNTIME_DIR/noctalia-$display.sock" ]; then
                export WAYLAND_DISPLAY="$display"
                break 2
            fi
        done
        sleep 0.1
    done
    for socket in "$XDG_RUNTIME_DIR"/niri.*.sock; do
        [ ! -S "$socket" ] || export NIRI_SOCKET="$socket"
    done
    snapshot() {
        name="$1"
        /usr/bin/niri msg action screenshot-screen --path "$HOME/.cache/alpha-$name.png"
        for n in $(seq 1 50); do [ -s "$HOME/.cache/alpha-$name.png" ] && break; sleep 0.1; done
        (
        echo "LUNA-ALPHA-IMAGE-BEGIN:$name"
        sha256sum "$HOME/.cache/alpha-$name.png" | cut -d ' ' -f 1 | while read -r checksum; do
            printf 'LUNA-ALPHA-SHA:%s:%s\n' "$name" "$checksum"
        done
        # Отдельный UART не перемешивает данные с журналом. Передаём одним
        # потоком, без десятков тысяч shell-вызовов для строк по 76 символов.
        printf 'LUNA-ALPHA-DATA:%s:' "$name"
        base64 -w 0 "$HOME/.cache/alpha-$name.png"
        printf '\n'
        echo "LUNA-ALPHA-IMAGE-END:$name"
        ) > /dev/ttyS1
    }
    sleep 3
    /usr/bin/noctalia config validate
    echo 'LUNA-ALPHA: Noctalia configuration valid'
    snapshot desktop
    echo 'LUNA-ALPHA: desktop absolute pointer ready'
    sleep 2
    snapshot pointer-launcher
    echo 'LUNA-ALPHA: desktop absolute pointer complete'
    /usr/bin/noctalia msg panel-toggle launcher
    mkdir -p "$HOME/alpha-open"
    printf 'LUNA-ALPHA: real selected file\n' > "$HOME/alpha-open/note.txt"
    for folder in Documents Downloads Pictures Music; do test -d "$HOME/$folder"; done
    echo 'LUNA-ALPHA: standard home directories present'
    /usr/bin/luna-files > "$HOME/.cache/alpha-files.log" 2>&1 &
    sleep 3
    files_pid=$(pidof luna-files)
    [ "$(echo "$files_pid" | wc -w)" = 1 ]
    snapshot files
    /usr/bin/luna-terminal > "$HOME/.cache/alpha-terminal.log" 2>&1 &
    sleep 4
    /usr/bin/niri msg -j windows
    echo 'LUNA-ALPHA: terminal keyboard ready'
    for n in $(seq 1 160); do [ -f "$HOME/alpha-terminal-verified" ] && break; sleep 0.1; done
    grep -Fxq alpha-real-fish "$HOME/alpha-terminal-verified"
    snapshot applications
    /usr/bin/niri msg action close-window
    sleep 1
    echo 'LUNA-ALPHA: file chooser keyboard ready'
    for n in $(seq 1 100); do
        /usr/bin/niri msg -j windows | grep -q 'Открыть с помощью' && break
        sleep 0.1
    done
    /usr/bin/niri msg -j windows | grep -q 'Открыть с помощью'
    [ "$(/usr/bin/niri msg -j windows | grep -o '"title":"Открыть с помощью…"' | wc -l)" = 1 ]
    snapshot file-chooser
    /usr/bin/niri msg -j windows
    echo 'LUNA-ALPHA: file chooser accept ready'
    for n in $(seq 1 160); do
        viewer=$(pidof less || true)
        [ -z "$viewer" ] || break
        sleep 0.1
    done
    if [ -z "$viewer" ]; then
        snapshot file-open-failure
        cat "$HOME/.cache/alpha-files.log"
        exit 1
    fi
    [ "$(echo "$viewer" | wc -w)" = 1 ]
    tr '\000' '\n' < "/proc/$viewer/cmdline" | grep -Fxq /open/selected
    cmp "$HOME/alpha-open/note.txt" "/proc/$viewer/root/open/selected"
    if printf 'invalid write\n' >> "/proc/$viewer/root/open/selected"; then
        echo 'LUNA-ALPHA: selected file unexpectedly writable'
        exit 1
    fi
    [ ! -e "/proc/$viewer/root/home/alpha-open/note.txt" ]
    [ ! -e "/proc/$viewer/root/run/luna/application.sock" ]
    sleep 3
    if /usr/bin/niri msg -j windows | grep -q 'Открыть с помощью'; then
        echo 'LUNA-ALPHA: file chooser remains after response'
        /usr/bin/niri msg -j windows
        snapshot file-dialog-failure
        exit 1
    fi
    snapshot selected-file
    echo 'LUNA-ALPHA: selected file read-only isolation verified'
    echo 'LUNA-ALPHA: viewer diagnostics'
    tr '\000' '\n' < "/proc/$viewer/cmdline"
    cat "/proc/$viewer/wchan"
    /usr/bin/niri msg -j windows
    echo 'LUNA-ALPHA: detached viewer keyboard ready'
    for n in $(seq 1 100); do [ ! -d "/proc/$files_pid" ] && break; sleep 0.1; done
    [ ! -d "/proc/$files_pid" ]
    kill -0 "$viewer"
    /usr/bin/niri msg -j windows
    snapshot detached-viewer
    echo 'LUNA-ALPHA: detached viewer survived Files exit'
    echo 'LUNA-ALPHA: viewer exit keyboard ready'
    for n in $(seq 1 100); do [ ! -d "/proc/$viewer" ] && break; sleep 0.1; done
    [ ! -d "/proc/$viewer" ]
    echo 'LUNA-ALPHA: viewer completed'
    /usr/bin/noctalia msg panel-toggle launcher
    sleep 2
    snapshot launcher
    /usr/bin/noctalia msg panel-toggle launcher
    /usr/bin/noctalia msg panel-toggle control-center
    sleep 2
    snapshot quick-settings
    /usr/bin/noctalia msg panel-toggle control-center
    echo 'LUNA-ALPHA: COMPLETE'
) >&2 &
'''


def prepare(tree):
    # Отдельный UART относится только к QEMU-проверке. Kernel/Noctalia пишут
    # журнал в первый порт, поэтому двоичные снимки во втором не перемешиваются.
    runtime = tree/'apps/luna-system-runtime/luna-system-runtime'
    observer = tree/'apps/luna-alpha-evidence'
    observer.mkdir()
    shutil.copyfile(runtime, observer/'luna-system-runtime-real')
    (observer/'luna-system-runtime-real').chmod(0o755)
    runtime.write_text('#!/bin/sh\nset -eu\nchmod 0666 /dev/ttyS1\nexec /usr/bin/luna-system-runtime-real "$@"\n')
    runtime.chmod(0o755)
    session = tree / 'apps/luna-graphical-session/luna-graphical-session'
    marker = '    echo "luna-graphical-session: starting Niri" >&2\n'
    text = session.read_text()
    if text.count(marker) != 1:
        raise ValueError('graphical session fixture injection mismatch')
    session.write_text(text.replace(marker, SESSION.replace("'", "'\"'\"'") + marker))
    subprocess.run(['sh', '-n', str(session)], check=True)


def send_keys(path, keys):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.connect(str(path))
        for key in keys:
            sock.sendall(f'sendkey {key} 30\n'.encode())
            time.sleep(.075)


def qmp_command(qmp_path, command, arguments=None):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(5)
        sock.connect(str(qmp_path))
        with sock.makefile('rwb', buffering=0) as stream:
            json.loads(stream.readline())
            for payload in [{'execute': 'qmp_capabilities'},
                            {'execute': command, 'arguments': arguments or {}}]:
                stream.write((json.dumps(payload)+'\n').encode())
                while True:
                    response = json.loads(stream.readline())
                    if 'return' in response or 'error' in response:
                        break
                if 'error' in response:
                    raise RuntimeError('QMP command failed: ' + str(response))
    return response['return']


def dump(qmp_path, path):
    try:
        qmp_command(qmp_path, 'screendump', {'filename': str(path)})
    except RuntimeError:
        return False
    return path.is_file()


def absolute_pointer(qmp_path, x, y, width, height, click=False):
    # QMP использует диапазон 0..32767. В госте пределы читаются через
    # EVIOCGABS; тест проверяет фактические пиксели после обработки evdev.
    events = [{'type': 'abs', 'data': {'axis': axis, 'value': round(value * 32767 / (extent - 1))}}
              for axis, value, extent in [('x', x, width), ('y', y, height)]]
    qmp_command(qmp_path, 'input-send-event', {'events': events})
    if click:
        for down in [True, False]:
            qmp_command(qmp_path, 'input-send-event', {'events': [
                {'type': 'btn', 'data': {'button': 'left', 'down': down}}]})
            time.sleep(.05)


def verify_native_pointer(qmp_path, root):
    from PIL import Image
    with Image.open(root/'login.ppm') as frame:
        width, height = frame.size
    previous = None
    for step in range(6):
        x, y = width * 3 // 4 + step * 8, height // 5
        absolute_pointer(qmp_path, x, y, width, height)
        time.sleep(.08)
        path = root/'login-pointer.ppm'
        if not dump(qmp_path, path): raise RuntimeError('native pointer scanout unavailable')
        with Image.open(path) as frame:
            if frame.getpixel((x, y)) != (233, 241, 255):
                raise RuntimeError(f'absolute pointer did not reach pixel {x},{y}')
            if previous and frame.getpixel(previous) == (233, 241, 255):
                raise RuntimeError('previous pointer was not erased')
        previous = (x, y)
    with Image.open(root/'login-pointer.ppm') as frame:
        frame.save(root/'login-pointer.png')


def capture_native(image, root):
    # QMP читает CPU scanout обычного virtio-vga. При virtio-vga-gl даже dumb
    # framebuffer может не иметь доступной QMP поверхности. Нативная графическая
    # граница и тот же образ остаются настоящими; отдельный запуск не вводит
    # экспортёр кадров или искусственную задержку в ОС.
    shutil.copyfile('/usr/share/OVMF/OVMF_VARS_4M.fd', root/'vars-native.fd')
    with tempfile.TemporaryDirectory(prefix='luna-alpha-native-', dir='/tmp') as control:
        hmp, qmp = Path(control)/'h.sock', Path(control)/'q.sock'
        serial = root/'serial-native.log'
        with (root/'qemu-native.log').open('wb') as output:
            process = subprocess.Popen(['qemu-system-x86_64', '-machine', 'q35', '-m', '2G', '-smp', '4',
                '-accel', 'kvm', '-cpu', 'host', '-vga', 'none', '-device', 'virtio-vga',
                '-device', 'qemu-xhci,id=pointer-usb', '-device', 'usb-tablet,bus=pointer-usb.0',
                '-drive', 'if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd',
                '-drive', f'if=pflash,format=raw,file={root}/vars-native.fd',
                '-drive', f'format=raw,snapshot=on,file={image}', '-monitor', f'unix:{hmp},server=on,wait=off',
                '-qmp', f'unix:{qmp},server=on,wait=off', '-serial', f'file:{serial}',
                '-display', 'none', '-no-reboot', '-no-shutdown'], stdout=output, stderr=output)
            try:
                deadline = time.monotonic()+45
                sent = False
                first_boot = None
                while time.monotonic()<deadline:
                    if process.poll() is not None: raise RuntimeError('native screenshot QEMU exited')
                    text = serial.read_text(errors='replace') if serial.exists() else ''
                    if qmp.exists() and 'animated boot presentation ready' in text and 'login UI first frame drawn' not in text:
                        if not (root/'boot.ppm').exists():
                            if dump(qmp, root/'boot.ppm'): first_boot = time.monotonic()
                        elif first_boot is not None and time.monotonic()-first_boot>.15 and not (root/'boot-motion.ppm').exists():
                            dump(qmp, root/'boot-motion.ppm')
                    if 'login UI keyboard/pointer input ready' in text and not sent:
                        if not dump(qmp, root/'login.ppm'): raise RuntimeError('native scanout unavailable')
                        if 'login absolute pointer ready' not in text:
                            raise RuntimeError('native login did not recognize absolute pointer')
                        verify_native_pointer(qmp, root)
                        send_keys(hmp, ['x'])
                        # Настоящий клик кнопки входа запускает проверку неверного
                        # пароля; Enter здесь не подменяет обработку указателя.
                        from PIL import Image
                        with Image.open(root/'login.ppm') as frame:
                            w, h = frame.size
                        absolute_pointer(qmp, w * 12 // 100 + 311, (h - 420) // 2 + 324, w, h, click=True)
                        sent = True
                    if 'authentication rejected' in text:
                        time.sleep(.2)
                        if not dump(qmp, root/'login-error.ppm'): raise RuntimeError('native error scanout unavailable')
                        return
                    time.sleep(.03)
                raise RuntimeError('native screenshot verification timed out')
            finally:
                process.terminate()
                try: process.wait(timeout=5)
                except subprocess.TimeoutExpired: process.kill(); process.wait()


def main():
    def cancelled(signum, frame):
        raise KeyboardInterrupt(f'Alpha verification cancelled by signal {signum}')
    signal.signal(signal.SIGTERM, cancelled)
    image = artifacts.regular(os.environ['LUNA_CANONICAL_IMAGE'])
    root = Path(os.environ['LUNA_CANONICAL_TEST_ROOT']).absolute()
    root.mkdir(parents=True, exist_ok=True)
    overlay = root / 'alpha.qcow2'
    before = artifacts.digest(image)
    process = None
    try:
        capture_native(image, root)
        patch = artifacts.patch_system_image(image, [], overlay=overlay, scratch=root, prepare_test_tree=prepare)
        (root/'patch.json').write_text(json.dumps(patch, indent=2))
        shutil.copyfile('/usr/share/OVMF/OVMF_VARS_4M.fd', root/'vars.fd')
        with tempfile.TemporaryDirectory(prefix='luna-alpha-', dir='/tmp') as control:
            hmp, qmp = Path(control)/'h.sock', Path(control)/'q.sock'
            serial = root/'serial.log'
            with (root/'qemu.log').open('wb') as output:
                process = subprocess.Popen(['qemu-system-x86_64', '-name', 'Luna Alpha design verification',
                    '-machine', 'q35', '-m', '2G', '-smp', '4', '-accel', 'kvm', '-cpu', 'host', '-vga', 'none',
                    '-device', os.environ.get('LUNA_QEMU_GPU', 'virtio-vga-gl'), '-nic', 'user,model=virtio-net-pci',
                    '-device', 'qemu-xhci,id=pointer-usb', '-device', 'usb-tablet,bus=pointer-usb.0',
                    '-drive', 'if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd',
                    '-drive', f'if=pflash,format=raw,file={root}/vars.fd', '-drive', f'format=qcow2,file={overlay}',
                    '-monitor', f'unix:{hmp},server=on,wait=off', '-qmp', f'unix:{qmp},server=on,wait=off',
                    '-serial', f'file:{serial}', '-serial', f'file:{root}/screenshots.log',
                    '-display', os.environ.get('LUNA_QEMU_DISPLAY', 'egl-headless'),
                    '-no-reboot', '-no-shutdown'], stdout=output, stderr=output)
                deadline = time.monotonic()+600
                sent_good = sent_terminal = sent_pointer = False
                sent_file = sent_accept = sent_detach = sent_exit = False
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError('QEMU exited; see qemu.log')
                    text = serial.read_text(errors='replace') if serial.exists() else ''
                    if 'Kernel panic' in text or 'session UI I/O failed' in text:
                        raise RuntimeError('guest failed; see serial.log')
                    if re.search(r'LUNA-ALPHA: observer exit [1-9]', text):
                        raise RuntimeError('Alpha observer failed; see serial.log and screenshots.log')
                    if "custom palette 'luna-alpha' not found or invalid" in text:
                        raise RuntimeError('Noctalia rejected the Alpha palette')
                    if 'login UI keyboard/pointer input ready' in text and not sent_good:
                        send_keys(hmp, ['l','u','n','a','ret'])
                        sent_good = True
                    if 'LUNA-ALPHA: desktop absolute pointer ready' in text and not sent_pointer:
                        # Настоящая кнопка меню на панели; никакого IPC открытия
                        # до этого снимка нет. Проверяем USB tablet и libinput/niri.
                        absolute_pointer(qmp, 29, 31, 1280, 800, click=True)
                        sent_pointer = True
                    if 'LUNA-ALPHA: terminal keyboard ready' in text and not sent_terminal:
                        line = 'echo alpha-real-fish > /home/alpha-terminal-verified'
                        symbols = {' ': 'spc', '-': 'minus', '>': 'shift-dot', '/': 'slash'}
                        send_keys(hmp, [symbols.get(ch,ch) for ch in line]+['ret'])
                        sent_terminal = True
                    if 'LUNA-ALPHA: file chooser keyboard ready' in text and not sent_file:
                        # Навигация и диалог — через настоящий ввод в GTK Files.
                        send_keys(hmp, ['ctrl-l'])
                        send_keys(hmp, ['slash','h','o','m','e','slash','a','l','p','h','a','minus','o','p','e','n','ret'])
                        time.sleep(1)
                        absolute_pointer(qmp, 310, 209, 1280, 800, click=True)
                        send_keys(hmp, ['ctrl-o', 'ctrl-o'])
                        sent_file = True
                    if 'LUNA-ALPHA: file chooser accept ready' in text and not sent_accept:
                        send_keys(hmp, ['ret'])
                        sent_accept = True
                    if 'LUNA-ALPHA: detached viewer keyboard ready' in text and not sent_detach:
                        # Закрываем именно Files; обработчик остаётся отдельным
                        # ApplicationInstance под контролем System Runtime.
                        send_keys(hmp, ['meta_l-left','meta_l-q'])
                        sent_detach = True
                    if 'LUNA-ALPHA: viewer exit keyboard ready' in text and not sent_exit:
                        send_keys(hmp, ['q'])
                        sent_exit = True
                    if 'LUNA-ALPHA: COMPLETE' in text:
                        break
                    time.sleep(.05)
                else:
                    raise RuntimeError('Alpha visual verification timed out; see serial.log')
                expected = ['login credentials accepted', 'boot presentation released DRM',
                    'LUNA-ALPHA: Noctalia configuration valid', 'org.luna.files', 'org.luna.terminal',
                    'LUNA-ALPHA: standard home directories present',
                    'LUNA-ALPHA: selected file read-only isolation verified',
                    'LUNA-ALPHA: detached viewer survived Files exit', 'LUNA-ALPHA: viewer completed']
                for marker in expected:
                    if marker not in text: raise RuntimeError('missing acceptance: '+marker)
                pointer_stage = text.split('LUNA-ALPHA: desktop absolute pointer complete')[0]
                if 'panel manager: opened "launcher"' not in pointer_stage:
                    raise RuntimeError('desktop absolute pointer did not open the launcher before IPC actions')
                captures = (root/'screenshots.log').read_text()
                for name in ['desktop','pointer-launcher','files','applications','file-chooser','selected-file','detached-viewer','launcher','quick-settings']:
                    match = re.search(r'LUNA-ALPHA-IMAGE-BEGIN:'+name+r'\n(.*?)LUNA-ALPHA-IMAGE-END:'+name, captures, re.S)
                    if not match: raise RuntimeError('missing screenshot: '+name)
                    encoded = ''.join(re.findall(r'^LUNA-ALPHA-DATA:'+name+r':([A-Za-z0-9+/=]+)$', match.group(1), re.M))
                    data = base64.b64decode(encoded, validate=True)
                    import hashlib
                    checksum = re.search(r'LUNA-ALPHA-SHA:'+name+r':([a-f0-9]{64})', match.group(1))
                    if checksum is None or hashlib.sha256(data).hexdigest() != checksum.group(1):
                        raise RuntimeError('screenshot transfer checksum mismatch: '+name)
                    if not data.startswith(b'\x89PNG\r\n\x1a\n'): raise RuntimeError('invalid screenshot: '+name)
                    (root/(name+'.png')).write_bytes(data)
                from PIL import Image
                for name in ['desktop','files','applications','launcher','quick-settings']:
                    with Image.open(root/(name+'.png')) as frame:
                        frame.verify()
                with Image.open(root/'desktop.png') as desktop, Image.open(root/'pointer-launcher.png') as menu:
                    region = (320, 160, 960, 640)
                    if desktop.crop(region).tobytes() == menu.crop(region).tobytes():
                        raise RuntimeError('desktop absolute pointer click did not open the launcher')
                for name in ['boot','boot-motion','login','login-error']:
                    path = root/(name+'.ppm')
                    if not path.is_file(): raise RuntimeError('missing native framebuffer screenshot: '+name)
                    Image.open(path).save(root/(name+'.png'))
                with Image.open(root/'boot.png') as first, Image.open(root/'boot-motion.png') as second:
                    w, h = first.size
                    region = (w//2-175, h//2-240, w//2+175, h//2+110)
                    if first.crop(region).tobytes() == second.crop(region).tobytes():
                        raise RuntimeError('boot rings did not change between real frames')
                (root/'results.txt').write_text('Настоящий отказ неверному паролю, успешный вход, освобождение DRM: PASS\n'
                    'Абсолютный указатель: шесть точных перемещений и настоящий клик входа: PASS\n'
                    'USB tablet и libinput/niri: настоящий клик открывает меню приложений: PASS\n'
                    'Стандартные каталоги HOME, GUI выбора файла, less, запрет записи/HOME/IPC, отдельный lifecycle: PASS\n'
                    'Noctalia configuration, desktop, Files, Ghostty/fish command, launcher, quick settings: PASS\n'
                    'Движение колец между настоящими кадрами, снимки настоящих экранов QEMU: PASS\nCanonical Luna Alpha design: PASS\n')
                print('Canonical Luna Alpha design: PASS')
    finally:
        if process is not None:
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
        overlay.unlink(missing_ok=True)
        if artifacts.digest(image) != before: raise RuntimeError('source image changed')

if __name__ == '__main__':
    main()
