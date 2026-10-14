#!/usr/bin/env python3
"""Real installed Bundle IPC/namespace/logout acceptance in a disposable QEMU layer."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import tomllib

import importlib.util
SPEC = importlib.util.spec_from_file_location("artifacts", Path(__file__).with_name("canonical-luna-artifacts.py"))
artifacts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(artifacts)
REPO = Path(__file__).resolve().parent.parent

SESSION = r'''(
    sleep 30
    set -eu
    failed_fixture() {
        result=$?
        [ "$result" -ne 0 ] || return 0
        set +e
        echo "LUNA-BUNDLES: fixture failure details status=$result terminal=${terminal_pid:-} fish=${fish_pid:-}"
        if [ -n "${copied_stage:-}" ]; then
            # Диагностика отличает ошибку отмены от завершения копирования
            # раньше доставки Escape управляющим процессом QEMU.
            echo "LUNA-DESKTOP: cancellation stage=$copied_stage"
            ls -l "$copied_stage" "$HOME/files-ops/copy/cancel.bin" "$HOME/files-ops/large/cancel.bin" 2>&1
            find "$HOME/files-ops/copy" -maxdepth 2 -name '.luna-*-copy'
        fi
        /usr/bin/niri msg action screenshot-screen --path "$HOME/.cache/files-error.png" || true
        sleep 1
        if [ -f "$HOME/.cache/files-error.png" ]; then
            echo 'LUNA-SCREENSHOT-BEGIN'
            base64 "$HOME/.cache/files-error.png"
            echo 'LUNA-SCREENSHOT-END'
        fi
        for pid in "${terminal_pid:-}" "${fish_pid:-}"; do
            [ -n "$pid" ] || continue
            grep -E '^(Name|Uid|Gid):' "/proc/$pid/status"
            cat "/proc/$pid/cgroup"
            readlink "/proc/$pid/ns/mnt"
            grep devpts "/proc/$pid/mountinfo"
            readlink "/proc/$pid/fd/0"
        done
        echo "LUNA-BUNDLES: fixture failed status=$result"
        exit "$result"
    }
    trap failed_fixture EXIT
    private_pty() {
        pid="$1"
        path="$(readlink "/proc/$pid/fd/0")"
        number="${path##*/}"
        case "$number" in ''|*[!0-9]*) return 1 ;; esac
        node="/proc/$pid/root/dev/pts/$number"
        [ -c "$node" ]
        # A reader outside the chroot sees its full mountpoint prefix. Compare
        # device/inode with the slave in this process's own root, not its spelling.
        [ "$(stat -Lc '%d:%i' "/proc/$pid/fd/0")" = "$(stat -Lc '%d:%i' "$node")" ]
    }
    probe_data="/data/users/luna/data/org.luna.launch-probe"
    check_dead() {
        pid="$(cat "$1")"
        if kill -0 "$pid" 2>/dev/null && ! grep -Eq "^State:[[:space:]]+[ZX]" "/proc/$pid/status"; then
            echo "LUNA-BUNDLES: descendant survived" >&2
            exit 1
        fi
    }
    run_status() {
        expected="$1"; shift
        set +e
        /usr/bin/luna app run "$@"
        status=$?
        set -e
        [ "$status" -eq "$expected" ] || { echo "LUNA-BUNDLES: wrong exit $status expected $expected"; exit 1; }
    }
    if [ -e "$HOME/.cache/luna-bundle-logout" ]; then
        run_status 0 org.luna.launch-probe 0.1.0 -- read
        run_status 0 org.luna.dynamic-probe 0.1.0 -- read
        if [ -e "$HOME/.cache/luna-bundle-reboot" ]; then
            echo "LUNA-BUNDLES: installed registry boots and data/config persists after reboot"
            echo "LUNA-BUNDLES: COMPLETE"
        else
            check_dead "$probe_data/hold.pid"
            if [ -e "$HOME/.cache/luna-wayland.pid" ]; then
                check_dead "$HOME/.cache/luna-wayland.pid"
                echo "LUNA-BUNDLES: Wayland application reclaimed on logout"
            fi
            if [ -e "$HOME/.cache/luna-terminal.pid" ]; then
                check_dead "$HOME/.cache/luna-terminal.pid"
                check_dead "$HOME/.cache/luna-fish.pid"
                [ -d "$HOME/gui-test" ]
                grep -Fxq fish-interactive "$HOME/fish-accepted"
                echo "LUNA-DESKTOP: terminal descendants reclaimed and home persists"
            fi
            echo "LUNA-BUNDLES: logout descendants reclaimed and data/config persisted"
            touch "$HOME/.cache/luna-bundle-reboot"
            for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
                display="${socket##*/}"
                if [ -S "$socket" ] && [ -S "$XDG_RUNTIME_DIR/noctalia-$display.sock" ]; then
                    export WAYLAND_DISPLAY="$display"
                    echo "LUNA-BUNDLES: requesting guest reboot with installed registry"
                    /usr/bin/noctalia msg session reboot
                    exit 0
                fi
            done
            exit 1
        fi
    else
        run_status 1 org.luna.launch-denied 0.1.0 -- write
        [ ! -e /data/users/luna/data/org.luna.launch-denied ]
        echo "LUNA-BUNDLES: ungranted content denied without data creation"
        run_status 1 org.luna.dynamic-denied 0.1.0 -- write
        [ ! -e /data/users/luna/data/org.luna.dynamic-denied ]
        run_status 17 org.luna.dynamic-probe 0.1.0 -- write
        run_status 0 org.luna.dynamic-probe 0.1.0 -- read
        echo "LUNA-BUNDLES: dynamic musl closure and missing-library grant denial verified"
        run_status 17 org.luna.launch-probe 0.1.0 -- write
        run_status 0 org.luna.launch-probe 0.1.0 -- read
        echo "LUNA-BUNDLES: credentials isolation persistence and exit status verified"
        run_status 143 org.luna.launch-probe 0.1.0 -- signal
        echo "LUNA-BUNDLES: signal status verified"
        run_status 0 org.luna.cli 0.1.0 -- --help
        echo "LUNA-BUNDLES: installed Luna CLI launched"
        for socket in "$XDG_RUNTIME_DIR"/niri.*.sock; do
            [ ! -S "$socket" ] || export NIRI_SOCKET="$socket"
        done
        if [ -x /usr/bin/luna-files ]; then
            # This helper is forked before Niri exports its child environment.
            # Match the real compositor and Noctalia sockets, not a guessed .0.
            display_count=0
            for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
                [ -S "$socket" ] || continue
                display="${socket##*/}"
                [ -S "$XDG_RUNTIME_DIR/noctalia-$display.sock" ] || continue
                WAYLAND_DISPLAY="$display"
                display_count=$((display_count + 1))
            done
            [ "$display_count" -eq 1 ]
            export WAYLAND_DISPLAY
            [ ! -e /data/users/luna/data/org.luna.files ]
            /usr/bin/luna app list > "$HOME/.cache/luna-catalog.toml"
            grep -Fxq 'application = "org.luna.files"' "$HOME/.cache/luna-catalog.toml"
            grep -Fxq 'application = "org.luna.terminal"' "$HOME/.cache/luna-catalog.toml"
            ! grep -Fq 'org.luna.weston-denied' "$HOME/.cache/luna-catalog.toml"
            ! grep -Fq 'org.luna.launch-probe' "$HOME/.cache/luna-catalog.toml"
            [ ! -e /data/users/luna/data/org.luna.files ]
            for entry in /run/luna/desktop-applications/share/applications/*.desktop; do
                [ "$(stat -c '%u:%a' "$entry")" = '0:444' ]
                grep -q '^Exec=/usr/bin/luna app run .* --expect-content [a-f0-9]* --$' "$entry"
            done
            [ ! -e /usr/share/applications/org.luna.files.desktop ]
            [ ! -e /usr/share/applications/org.luna.terminal.desktop ]
            set +e
            stale_output="$(/usr/bin/luna app run org.luna.files 0.1.0 --expect-content 0000000000000000000000000000000000000000000000000000000000000000 2>&1)"
            stale_status=$?
            set -e
            [ "$stale_status" -eq 1 ]
            printf '%s' "$stale_output" | grep -Fq 'content identity is stale'
            [ ! -e /data/users/luna/data/org.luna.files ]
            echo 'LUNA-DESKTOP: verified catalog filters grants and rejects stale pin before storage'
            window_pid() {
                /usr/bin/niri msg --json windows > "$HOME/.cache/luna-desktop-windows.json"
                tr -d '\n' < "$HOME/.cache/luna-desktop-windows.json" | sed 's/},[[:space:]]*{/}\n{/g; s/"app_id":[[:space:]]*/"app_id":/g' | grep -F "\"app_id\":\"$1\"" | sed -n 's/.*"pid":[[:space:]]*\([0-9][0-9]*\).*/\1/p'
            }
            /usr/bin/noctalia msg panel-toggle launcher
            mkdir -p "$HOME/files-ops/source" "$HOME/files-ops/copy" "$HOME/files-ops/move"
            printf 'files-operation-content\n' > "$HOME/files-ops/source/note"
            ln -s note "$HOME/files-ops/source/z-link"
            echo 'LUNA-DESKTOP: Files menu ready'
            files_pid=""
            for n in $(seq 1 150); do
                files_pid="$(window_pid org.luna.files)"
                [ -z "$files_pid" ] || break
                sleep 0.2
            done
            if [ -z "$files_pid" ] || [ "$files_pid" = null ]; then
                cat "$HOME/.cache/luna-desktop-windows.json"
                echo 'LUNA-DESKTOP: Files window PID unavailable'
                exit 1
            fi
            grep -Eq '^Uid:[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000' "/proc/$files_pid/status"
            [ "$(readlink /proc/$files_pid/root)" != / ]
            echo 'LUNA-DESKTOP: Noctalia menu launched registered Files'
            echo "LUNA-DESKTOP: Files menu PID=$files_pid"
            cat "$HOME/.cache/luna-desktop-windows.json"
            echo 'LUNA-DESKTOP: Files input ready'
            for n in $(seq 1 150); do
                [ ! -d "$HOME/gui-test" ] || break
                sleep 0.2
            done
            [ -d "$HOME/gui-test" ]
            echo 'LUNA-DESKTOP: graphical Yazi navigation and folder creation verified'
            wait_file() {
                for n in $(seq 1 200); do [ -e "$1" ] && return 0; sleep 0.1; done
                echo "LUNA-DESKTOP: missing operation result $1"; return 1
            }
            echo 'LUNA-DESKTOP: Files copy input ready'
            wait_file "$HOME/files-ops/copy/note"
            cmp "$HOME/files-ops/source/note" "$HOME/files-ops/copy/note"
            # Preserve both files on an existing destination, then rename.
            printf 'destination-conflict-keep\n' > "$HOME/files-ops/copy/note"
            cp "$HOME/files-ops/copy/note" "$HOME/.cache/expected-files-destination"
            echo 'LUNA-DESKTOP: Files rename input ready'
            wait_file "$HOME/files-ops/copy/renamed"
            [ ! -e "$HOME/files-ops/copy/note" ]
            cmp "$HOME/.cache/expected-files-destination" "$HOME/files-ops/copy/renamed"
            echo 'LUNA-DESKTOP: Files move input ready'
            wait_file "$HOME/files-ops/move/renamed"
            [ ! -e "$HOME/files-ops/copy/renamed" ]
            cmp "$HOME/.cache/expected-files-destination" "$HOME/files-ops/move/renamed"
            echo 'LUNA-DESKTOP: Files trash input ready'
            for n in $(seq 1 200); do [ ! -e "$HOME/files-ops/move/renamed" ] && break; sleep 0.1; done
            [ ! -e "$HOME/files-ops/move/renamed" ]
            trash_info="$(find "$HOME/.local/share/Trash/info" -name '*.trashinfo' -type f)"
            [ -n "$trash_info" ]
            grep -qx 'Path=/home/files-ops/move/renamed' "$trash_info"
            grep -Eq '^DeletionDate=[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}$' "$trash_info"
            trash_key="$(basename "$trash_info" .trashinfo)"
            cmp "$HOME/.cache/expected-files-destination" "$HOME/.local/share/Trash/files/$trash_key"
            echo 'LUNA-DESKTOP: Files recover input ready'
            wait_file "$HOME/files-ops/move/restored"
            cmp "$HOME/.cache/expected-files-destination" "$HOME/files-ops/move/restored"
            [ ! -e "$HOME/.local/share/Trash/files/$trash_key" ]
            echo 'LUNA-DESKTOP: Files directory copy input ready'
            wait_file "$HOME/files-ops/copy/source/note"
            cmp "$HOME/files-ops/source/note" "$HOME/files-ops/copy/source/note"
            [ "$(readlink "$HOME/files-ops/copy/source/z-link")" = note ]
            echo 'LUNA-DESKTOP: GUI copy conflict rename move Trash recovery and recursive links verified'
            mkdir -p "$HOME/files-ops/large"
            truncate -s 512M "$HOME/files-ops/large/cancel.bin"
            echo 'LUNA-DESKTOP: Files cancel-start input ready'
            copied_stage=""
            for n in $(seq 1 400); do
                copied_stage="$(find "$HOME/files-ops/copy" -maxdepth 2 -path '*/.luna-*-copy/payload' -type f -size +0c | head -n 1)"
                [ -z "$copied_stage" ] || break
                sleep 0.05
            done
            [ -n "$copied_stage" ]
            echo 'LUNA-DESKTOP: Files cancel input ready'
            for n in $(seq 1 200); do [ ! -e "$copied_stage" ] && break; sleep 0.1; done
            [ ! -e "$copied_stage" ]
            [ ! -e "$HOME/files-ops/copy/cancel.bin" ]
            [ "$(stat -c %s "$HOME/files-ops/large/cancel.bin")" = 536870912 ]
            [ -z "$(find "$HOME/files-ops/copy" -maxdepth 1 -name '.luna-*-copy')" ]
            rm "$HOME/files-ops/large/cancel.bin"
            echo 'LUNA-DESKTOP: GUI in-progress copy cancellation and staging cleanup verified'

            /usr/bin/niri msg action close-window
            for n in $(seq 1 100); do
                [ -e "/proc/$files_pid" ] || break
                sleep 0.2
            done
            [ ! -e "/proc/$files_pid" ]
            echo 'LUNA-DESKTOP: Files menu window reclaimed on close'
            /usr/bin/noctalia msg panel-toggle launcher
            echo 'LUNA-DESKTOP: Terminal menu ready'
            terminal_pid=""
            for n in $(seq 1 150); do
                terminal_pid="$(window_pid org.luna.terminal)"
                [ -z "$terminal_pid" ] || break
                sleep 0.2
            done
            if [ -z "$terminal_pid" ] || [ "$terminal_pid" = null ]; then
                cat "$HOME/.cache/luna-desktop-windows.json"
                echo 'LUNA-DESKTOP: Terminal window PID unavailable'
                exit 1
            fi
            echo "$terminal_pid" > "$HOME/.cache/luna-terminal.pid"
            echo 'LUNA-DESKTOP: Noctalia menu launched registered Terminal defaults'
            grep -Eq '^Uid:[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000' "/proc/$terminal_pid/status"
            grep -q ' /dev/pts .* devpts ' "/proc/$terminal_pid/mountinfo"
            [ ! -e "/proc/$terminal_pid/root/dev/dri" ]
            [ ! -e "/proc/$terminal_pid/root/dev/console" ]
            fish_pid=""
            for n in $(seq 1 100); do
                for name in /proc/[0-9]*/comm; do
                    [ -r "$name" ] || continue
                    if [ "$(cat "$name")" = fish ]; then
                        candidate="${name%/comm}"; candidate="${candidate##*/}"
                        if [ "$(readlink /proc/$candidate/ns/mnt)" = "$(readlink /proc/$terminal_pid/ns/mnt)" ]; then
                            fish_pid="$candidate"; break
                        fi
                    fi
                done
                [ -z "$fish_pid" ] || break
                sleep 0.2
            done
            [ -n "$fish_pid" ]
            echo "$fish_pid" > "$HOME/.cache/luna-fish.pid"
            cmp "/proc/$fish_pid/cgroup" "/proc/$terminal_pid/cgroup"
            private_pty "$fish_pid"
            echo 'LUNA-DESKTOP: Terminal input ready'
            for n in $(seq 1 200); do
                [ ! -f "$HOME/yazi-accepted" ] || break
                sleep 0.2
            done
            grep -Fxq fish-interactive "$HOME/fish-accepted"
            grep -Fxq bash-compatible "$HOME/bash-accepted"
            grep -Fxq sh-compatible "$HOME/sh-accepted"
            grep -Fxq Yazi "$HOME/yazi-accepted"
            grep -Eq '^[[:space:]]+Version:[[:space:]]+26\.9\.1([[:space:]]|$)' "$HOME/yazi-accepted"
            yazi_pid=""
            for n in $(seq 1 100); do
                for name in /proc/[0-9]*/comm; do
                    [ -r "$name" ] || continue
                    if [ "$(cat "$name")" = yazi ]; then
                        candidate="${name%/comm}"; candidate="${candidate##*/}"
                        if [ "$(readlink /proc/$candidate/ns/mnt)" = "$(readlink /proc/$terminal_pid/ns/mnt)" ]; then
                            yazi_pid="$candidate"; break
                        fi
                    fi
                done
                [ -z "$yazi_pid" ] || break
                sleep 0.2
            done
            [ -n "$yazi_pid" ]
            cmp "/proc/$yazi_pid/cgroup" "/proc/$terminal_pid/cgroup"
            private_pty "$yazi_pid"
            echo 'LUNA-DESKTOP: Yazi TUI input ready'
            for n in $(seq 1 150); do
                [ ! -f "$HOME/yazi-tui-accepted" ] || break
                sleep 0.2
            done
            grep -Fxq yazi-tui-exited "$HOME/yazi-tui-accepted"
            [ ! -e "/proc/$yazi_pid" ]
            kill -0 "$fish_pid"
            echo 'LUNA-DESKTOP: interactive Ghostty fish Bash sh Yazi and private PTY verified'
        fi
        if [ -e /usr/share/luna-launch-fixtures/wayland.identity ]; then
            run_status 1 org.luna.weston-denied 14.0.2
            [ ! -e /data/users/luna/data/org.luna.weston-denied ]
            echo "LUNA-BUNDLES: Wayland capability denied without data creation"
            for socket in "$XDG_RUNTIME_DIR"/niri.*.sock; do
                [ ! -S "$socket" ] || export NIRI_SOCKET="$socket"
            done
            [ -n "${NIRI_SOCKET:-}" ]
            if [ -x /usr/bin/luna-files ]; then
                grep -Fxq 'application = "org.luna.weston-simple-shm"' "$HOME/.cache/luna-catalog.toml"
                /usr/bin/noctalia msg panel-toggle launcher
                echo 'LUNA-DESKTOP: Weston menu ready'
            else
                /usr/bin/luna app run org.luna.weston-simple-shm 14.0.2 &
                client=$!
            fi
            pid=""
            for n in $(seq 1 100); do
                /usr/bin/niri msg --json windows > "$HOME/.cache/luna-wayland-windows.json"
                pid="$(tr -d "\n" < "$HOME/.cache/luna-wayland-windows.json" | sed "s/},[[:space:]]*{/}\n{/g" | grep -F "org.freedesktop.weston.simple-shm" | sed -n "s/.*\"pid\":[[:space:]]*\([0-9][0-9]*\).*/\1/p")"
                [ -z "$pid" ] || break
                if [ ! -x /usr/bin/luna-files ]; then
                    kill -0 "$client" || { echo "LUNA-BUNDLES: Wayland app exited before window"; exit 1; }
                fi
                sleep 0.2
            done
            [ -n "$pid" ] && [ "$pid" != "null" ]
            echo "$pid" > "$HOME/.cache/luna-wayland.pid"
            if [ -x /usr/bin/luna-files ]; then
                echo 'LUNA-DESKTOP: Noctalia menu launched independent registered Weston fixture'
            fi
            grep -Eq "^Uid:[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000[[:space:]]+1000" "/proc/$pid/status"
            grep -Fq "libwayland-client.so.0" "/proc/$pid/maps"
            grep -Fq "ld-musl-x86_64.so.1" "/proc/$pid/maps"
            [ "$(readlink /proc/$pid/root)" != "/" ]
            cat "$HOME/.cache/luna-wayland-windows.json"
            echo "LUNA-BUNDLES: Wayland window observed"
            sleep 3
            kill -0 "$pid"
            echo "LUNA-BUNDLES: real Weston Wayland window dynamic libraries and UID verified"
        fi
        /usr/share/luna-launch-fixtures/probe-canary serve-ipc &
        canary=$!
        for n in $(seq 1 30); do
            [ ! -e "$probe_data/ipc-ready" ] || break
            sleep 0.1
        done
        [ -s "$probe_data/ipc-ready" ]
        run_status 0 org.luna.launch-probe 0.1.0 -- ipc-deny
        echo "LUNA-BUNDLES: external named and abstract UNIX sockets denied"
        kill "$canary"
        wait "$canary" || true
        run_status 17 org.luna.launch-probe 0.1.0 -- detached
        check_dead "$probe_data/detached.pid"
        echo "LUNA-BUNDLES: setsid descendant reclaimed on leader exit"
        /usr/bin/luna app run org.luna.launch-probe 0.1.0 -- hold &
        for n in 1 2 3 4 5 6 7 8 9 10; do
            [ ! -e "$probe_data/hold.pid" ] || break
            sleep 0.2
        done
        [ -s "$probe_data/hold.pid" ]
        mkdir -p "$HOME/.cache"
        touch "$HOME/.cache/luna-bundle-logout"
        for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
            display="${socket##*/}"
            if [ -S "$socket" ] && [ -S "$XDG_RUNTIME_DIR/noctalia-$display.sock" ]; then
                export WAYLAND_DISPLAY="$display"
                echo "LUNA-BUNDLES: requesting logout with running application"
                /usr/bin/noctalia msg session logout
                exit 0
            fi
        done
        echo "LUNA-BUNDLES: Noctalia IPC missing"
        exit 1
    fi
) >&2 &
'''

def command(*args):
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, text=True).stdout

def fixtures(work):
    cli = REPO / "target/x86_64-unknown-linux-musl/release/luna"
    artifacts.regular(cli)
    payload = work / "payload"
    (payload / "bin").mkdir(parents=True)
    command("musl-gcc", "-static", "-O2", "-Wall", "-Wextra", "-Werror", REPO / "tools/fixtures/luna-launch-probe.c", "-o", payload / "bin/probe")
    policies = []
    names = ["launch-probe", "launch-denied", "cli", "dynamic-probe", "dynamic-denied"]
    dynamic = work / "dynamic-bin"
    dynamic.mkdir()
    command("musl-gcc", "-O2", "-Wall", "-Wextra", "-Werror", REPO / "tools/fixtures/luna-launch-probe.c", "-o", dynamic / "probe")
    loader = Path(os.environ["LUNA_CANONICAL_INPUT_ROOT"]) / "provider-root/lib/ld-musl-x86_64.so.1"
    artifacts.regular(loader)
    for name in names:
        app = f"org.luna.{name}"
        executable = "luna" if name == "cli" else "probe"
        source = work / name
        (source / "bin").mkdir(parents=True)
        shutil.copyfile(cli if name == "cli" else (dynamic / "probe" if name.startswith("dynamic-") else payload / "bin/probe"), source / "bin" / executable)
        (source / "bin" / executable).chmod(0o755)
        manifest = f'''format = 1
[bundle]
id = "{app}"
name = "{name}"
version = "0.1.0"
type = "application"
[platform]
arch = "x86_64"
[entry]
exec = "bin/{executable}"
logical = "/bin/{executable}"
[[mapping]]
logical = "/bin/{executable}"
source = "bin/{executable}"
access = ["read", "execute"]
'''
        libraries = []
        if name.startswith("dynamic-"):
            (source / "lib").mkdir()
            for library in ["ld-musl-x86_64.so.1", "libc.so"]:
                shutil.copyfile(loader, source / "lib" / library)
                (source / "lib" / library).chmod(0o755)
                libraries.append(f"/lib/{library}")
                manifest += f'\n[[mapping]]\nlogical = "/lib/{library}"\nsource = "lib/{library}"\naccess = ["read", "execute"]\n'
        path = work / f"{name}.toml"
        path.write_text(manifest)
        archive = work / f"{name}.lbp"
        command(cli, "bundle", "pack", path, source, archive)
        identity = re.search(r"content_identity=([a-f0-9]{64})", command(cli, "bundle", "inspect", archive)).group(1)
        (work / f"{name}.identity").write_text(identity)
        if name != "launch-denied":
            policies.append(f'''[[application]]
user = "luna"
id = "{app}"
version = "0.1.0"
content_identity = "{identity}"
runtime = "musl"
execute = true
private_data = true
[[application.path]]
logical = "/bin/{executable}"
access = ["read", "execute"]
[[application.path]]
logical = "/data"
access = ["read", "write"]
[[application.path]]
logical = "/config"
access = ["read", "write"]
''')
            for library in libraries:
                # One exact dependency permission is deliberately absent.
                if name == "dynamic-denied" and library.endswith("libc.so"):
                    continue
                policies[-1] += f'\n[[application.path]]\nlogical = "{library}"\naccess = ["read", "execute"]\n'
    wayland_fixture = os.environ.get("LUNA_CANONICAL_WAYLAND_FIXTURE")
    if wayland_fixture:
        import tomllib
        selected = Path(wayland_fixture)
        provenance = json.loads((selected / "provenance.json").read_text())
        expected_lock = artifacts.digest(Path(os.environ["LUNA_CANONICAL_INPUT_ROOT"]) / "inputs.lock.json")
        if provenance["input_lock_sha256"] != expected_lock:
            raise ValueError("Wayland fixture uses different canonical input snapshot")
        for resource, info in provenance["resources"].items():
            if artifacts.digest(selected / "payload" / resource) != info["payload_sha256"]:
                raise ValueError("Wayland fixture resource changed")
        manifest = (selected / "manifest.toml").read_text()
        declarations = tomllib.loads(manifest)["mapping"]
        for name in ["wayland", "wayland-denied"]:
            app = "org.luna.weston-simple-shm" if name == "wayland" else "org.luna.weston-denied"
            archive = work / f"{name}.lbp"
            path = work / f"{name}.toml"
            path.write_text(manifest.replace("org.luna.weston-simple-shm", app))
            command(cli, "bundle", "pack", path, selected / "payload", archive)
            identity = re.search(r"content_identity=([a-f0-9]{64})", command(cli, "bundle", "inspect", archive)).group(1)
            (work / f"{name}.identity").write_text(identity)
            grant = f'[[application]]\nuser = "luna"\nid = "{app}"\nversion = "14.0.2"\ncontent_identity = "{identity}"\nruntime = "musl"\nexecute = true\nprivate_data = true\n'
            if name == "wayland": grant += 'capabilities = ["desktop.wayland"]\n'
            for path_name, access in [(entry["logical"], '["read", "execute"]') for entry in declarations] + [("/data", '["read", "write"]'), ("/config", '["read", "write"]'), ("/run/wayland-0", '["read"]')]:
                grant += f'\n[[application.path]]\nlogical = "{path_name}"\naccess = {access}\n'
            policies.append(grant)
            names.append(name)
        shutil.copyfile(selected / "provenance.json", work / "wayland-provenance.json")
    (work / "names.json").write_text(json.dumps(names))
    (work / "application-launch.toml").write_text("format = 1\n" + "\n".join(policies))

def prepare_tree(tree, fixture):
    app = tree / "apps/luna-app-acceptance"
    resources = app / "resources/luna-launch-fixtures"
    resources.mkdir(parents=True)
    shutil.copyfile(fixture / "payload/bin/probe", resources / "probe-canary")
    (resources / "probe-canary").chmod(0o755)
    names = json.loads((fixture / "names.json").read_text())
    for name in names:
        for suffix in ("lbp", "identity"):
            shutil.copyfile(fixture / f"{name}.{suffix}", resources / f"{name}.{suffix}")
    shutil.copyfile(fixture / "application-launch.toml", resources / "application-launch.toml")
    if (fixture / "wayland-provenance.json").exists(): shutil.copyfile(fixture / "wayland-provenance.json", resources / "wayland-provenance.json")
    production_policy = Path(os.environ["LUNA_CANONICAL_INPUT_ROOT"]) / "desktop-bundles/application-launch.toml"
    if production_policy.exists():
        selected = tomllib.loads(production_policy.read_text())
        if selected.get("format") != 1: raise ValueError("unsupported desktop policy")
        with (resources / "application-launch.toml").open("a") as out:
            out.write("\n" + production_policy.read_text().split("format = 1\n", 1)[1])
    runtime = tree / "apps/luna-system-runtime/luna-system-runtime"
    shutil.copyfile(runtime, app / "luna-system-runtime-real")
    (app / "luna-system-runtime-real").chmod(0o755)
    runtime.write_text('''#!/bin/sh
set -eu
if [ ! -e /.luna/data/system/config/luna-launch-fixture-installed ]; then
for name in __FIXTURE_NAMES__; do
    identity="$(cat /usr/share/luna-launch-fixtures/$name.identity)"
    /usr/bin/luna app --offline-data-root /.luna/data install /usr/share/luna-launch-fixtures/$name.lbp --trust-content "$identity"
done
cp /usr/share/luna-launch-fixtures/application-launch.toml /.luna/data/system/config/application-launch.toml
chmod 0600 /.luna/data/system/config/application-launch.toml
touch /.luna/data/system/config/luna-launch-fixture-installed
echo "LUNA-BUNDLES: installed independent directory Bundles"
fi
(
    for n in $(seq 1 300); do
        [ ! -S /run/luna/application.sock ] || break
        sleep 0.2
    done
    [ -S /run/luna/application.sock ] || exit 1
    set +e
    output="$(/usr/bin/luna app run org.luna.launch-probe 0.1.0 -- write 2>&1)"
    result=$?
    echo "LUNA-BUNDLES: wrong UID rejection diagnostic: status=$result $output"
    [ "$result" -eq 1 ] && printf "%s" "$output" | grep -Fq "peer is not the active session user" && echo "LUNA-BUNDLES: wrong UID denied"
    output="$(su -s /bin/sh luna -c "/usr/bin/luna app run org.luna.launch-probe 0.1.0 -- write" 2>&1)"
    result=$?
    echo "LUNA-BUNDLES: same UID rejection diagnostic: status=$result $output"
    [ "$result" -eq 1 ] && printf "%s" "$output" | grep -Fq "peer is not the active session user" && echo "LUNA-BUNDLES: same UID outside session denied"
    output="$(/usr/bin/luna app list 2>&1)"; result=$?
    [ "$result" -eq 1 ] && printf '%s' "$output" | grep -Fq 'peer is not the active session user' && echo 'LUNA-DESKTOP: catalog wrong UID denied'
    output="$(su -s /bin/sh luna -c '/usr/bin/luna app list' 2>&1)"; result=$?
    [ "$result" -eq 1 ] && printf '%s' "$output" | grep -Fq 'peer is not the active session user' && echo 'LUNA-DESKTOP: catalog outside session denied'
) >&2 &
exec /usr/bin/luna-system-runtime-real "$@"
'''.replace("__FIXTURE_NAMES__", " ".join(names)))
    runtime.chmod(0o755)
    command("sh", "-n", runtime)
    session = tree / "apps/luna-graphical-session/luna-graphical-session"
    marker = '    echo "luna-graphical-session: starting Niri" >&2\n'
    text = session.read_text()
    if text.count(marker) != 1:
        raise ValueError("Bundle session fixture injection mismatch")
    # The production session embeds its body in a shell single-quoted string.
    # Escape literal quotes in the injected fixture without changing its bytes.
    session.write_text(text.replace(marker, SESSION.replace("'", "'\"'\"'") + marker))
    command("sh", "-n", session)

def login(monitor):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.connect(str(monitor))
        for key in ("l", "u", "n", "a", "ret"):
            sock.sendall(f"sendkey {key}\n".encode())
            time.sleep(.3)

def verify_files_menu_exit(text):
    window = re.search(r'LUNA-DESKTOP: Files menu PID=(\d+)', text)
    outcomes = re.finditer(r'application complete application=org\.luna\.files instance=(\d+) pid=(\d+) exit=Some\(0\) signal=None cleanup_ok=true', text)
    if not window or not any(window.group(1) == outcome.group(2) for outcome in outcomes):
        raise RuntimeError('Files menu instance did not exit cleanly with matching PID and cleanup')

def text_keys(text):
    symbols = {" ": "spc", "/": "slash", "-": "minus", ";": "semicolon", ".": "dot",
        ">": "shift-dot", '"': "shift-apostrophe", "_": "shift-minus", "=": "equal",
        ":": "shift-semicolon", "'": "apostrophe", "\\": "backslash"}
    result = []
    for character in text:
        if character in symbols: result.append(symbols[character])
        elif character.isascii() and character.isalnum():
            result.append('shift-' + character.lower() if character.isupper() else character)
        else: raise ValueError('unsupported virtual-keyboard character: ' + repr(character))
    return result

def desktop_keys(monitor, stage):
    keys = text_keys
    if stage == "files":
        sequence = ["ctrl-l", *keys("/home"), "ret", None, "ctrl-shift-n", None, *keys("gui-test"), "ret"]
    elif stage == "files-copy":
        sequence = ["ctrl-l", *keys("/home/files-ops/source"), "ret", None, "ctrl-c", "ctrl-l", *keys("/home/files-ops/copy"), "ret", None, "ctrl-v"]
    elif stage == "files-rename":
        sequence = [None, "ctrl-v", None, "f2", None, "ctrl-a", *keys("renamed"), "ret"]
    elif stage == "files-move":
        sequence = [None, "ctrl-x", "ctrl-l", *keys("/home/files-ops/move"), "ret", None, "ctrl-v"]
    elif stage == "files-trash":
        sequence = [None, "delete"]
    elif stage == "files-recover":
        sequence = ["ctrl-l", *keys("/home/.local/share/Trash/files"), "ret", None, "ctrl-x", "ctrl-l", *keys("/home/files-ops/move"), "ret", None, "ctrl-v", None, "f2", None, "ctrl-a", *keys("restored"), "ret"]
    elif stage == "files-directory-copy":
        sequence = ["ctrl-l", *keys("/home/files-ops"), "ret", None, "down", "down", "ctrl-c", "ctrl-l", *keys("/home/files-ops/copy"), "ret", None, "ctrl-v"]
    elif stage == "files-cancel-start":
        sequence = ["ctrl-l", *keys("/home/files-ops/large"), "ret", None, "ctrl-c", "ctrl-l", *keys("/home/files-ops/copy"), "ret", None, "ctrl-v"]
    elif stage == "files-cancel":
        sequence = ["esc"]
    elif stage == "yazi":
        sequence = [None, "q"]
    elif stage.endswith("-menu"):
        query = {"files-menu": "luna files", "terminal-menu": "terminal", "weston-menu": "weston simple shm"}[stage]
        sequence = [None, "ctrl-a", *keys(query), None, "ret"]
    else:
        line = 'status is-interactive; and echo fish-interactive >/home/fish-accepted; /bin/bash -c "echo bash-compatible" >/home/bash-accepted; /bin/sh -c "echo sh-compatible" >/home/sh-accepted; yazi --version >/home/yazi-accepted; yazi /home; and echo yazi-tui-exited >/home/yazi-tui-accepted'
        sequence = [None, *keys(line), "ret"]
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.connect(str(monitor)); sock.settimeout(.01)
        if stage != "files-cancel": time.sleep(1)
        for key in sequence:
            if key is None:
                time.sleep(1); continue
            sock.sendall(("sendkey " + key + " 10\n").encode()); time.sleep(.03)
            try: sock.recv(65536)
            except socket.timeout: pass

def main():
    def cancelled(signum, frame):
        raise KeyboardInterrupt(f"Bundle test cancelled by signal {signum}")
    signal.signal(signal.SIGTERM, cancelled)
    image = artifacts.regular(os.environ["LUNA_CANONICAL_IMAGE"])
    before = artifacts.digest(image)
    root = Path(os.environ["LUNA_CANONICAL_TEST_ROOT"]).absolute()
    root.mkdir(parents=True, exist_ok=True)
    overlay = root / "guest.qcow2"
    if overlay.exists() or overlay.is_symlink(): raise ValueError("test overlay already exists")
    qemu = None
    try:
        with tempfile.TemporaryDirectory(prefix="luna-app-launch-", dir=root) as temporary:
            fixture = Path(temporary)
            fixtures(fixture)
            patch = artifacts.patch_system_image(image, [], overlay=overlay, scratch=root,
                prepare_test_tree=lambda tree: prepare_tree(tree, fixture))
            (root / "patch.json").write_text(json.dumps(patch, indent=2))
        command("sync", "-f", overlay)
        variables = root / "vars.fd"
        shutil.copyfile("/usr/share/OVMF/OVMF_VARS_4M.fd", variables)
        with tempfile.TemporaryDirectory(prefix="luna-app-monitor-", dir="/tmp") as control:
            monitor = Path(control) / "h.sock"
            serial = root / "serial.log"
            with (root / "qemu.log").open("wb") as log:
                qemu = subprocess.Popen(["qemu-system-x86_64", "-name", "Luna installed Bundle acceptance", "-machine", "q35", "-m", "2G", "-smp", "4", "-accel", "kvm", "-cpu", "host", "-vga", "none", "-device", os.environ.get("LUNA_QEMU_GPU", "virtio-vga-gl"), "-nic", "user,model=virtio-net-pci", "-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd", "-drive", f"if=pflash,format=raw,file={variables}", "-drive", f"format=qcow2,file={overlay}", "-monitor", f"unix:{monitor},server=on,wait=off", "-serial", f"file:{serial}", "-display", os.environ.get("LUNA_QEMU_DISPLAY", "sdl,gl=on"), "-no-shutdown"], stdout=log, stderr=log)
                deadline = time.monotonic() + 480
                desktop_sent = set()
                sent = 0
                while time.monotonic() < deadline:
                    if qemu.poll() is not None: raise RuntimeError("QEMU exited during Bundle acceptance")
                    text = serial.read_text(errors="replace") if serial.exists() else ""
                    for failure in ("descendant survived", "wrong exit", "fixture failed", "application broker unavailable", "application broker failed", "session descendant boundary unavailable", "cgroup cleanup failed", "Kernel panic", "Wayland app exited before window", "Files exited before window", "Terminal exited before window", "window PID unavailable", "application child setup failed"):
                        if failure in text:
                            raise RuntimeError(failure)
                    count = text.count("luna-user-session: login UI keyboard/pointer input ready")
                    if count > sent and monitor.exists():
                        login(monitor); sent += 1
                    if "LUNA-DESKTOP: Files input ready" in text and "files" not in desktop_sent:
                        desktop_keys(monitor, "files"); desktop_sent.add("files")
                    for operation in ["copy", "rename", "move", "trash", "recover", "directory copy", "cancel-start", "cancel"]:
                        stage = "files-" + operation.replace(" ", "-")
                        if "LUNA-DESKTOP: Files " + operation + " input ready" in text and stage not in desktop_sent:
                            desktop_keys(monitor,stage); desktop_sent.add(stage)
                    if "LUNA-DESKTOP: Terminal input ready" in text and "terminal" not in desktop_sent:
                        desktop_keys(monitor, "terminal"); desktop_sent.add("terminal")
                    if "LUNA-DESKTOP: Yazi TUI input ready" in text and "yazi" not in desktop_sent:
                        desktop_keys(monitor, "yazi"); desktop_sent.add("yazi")
                    for stage, message in [("files-menu", "Files menu ready"), ("terminal-menu", "Terminal menu ready"), ("weston-menu", "Weston menu ready")]:
                        if "LUNA-DESKTOP: " + message in text and stage not in desktop_sent:
                            desktop_keys(monitor, stage); desktop_sent.add(stage)
                    if "LUNA-BUNDLES: COMPLETE" in text: break
                    # После начала большой копии ожидаем сигнал отмены чаще:
                    # быстрый Btrfs может закончить 512 MiB за обычные 200 мс.
                    # Сами условия проверки отмены и очистки не меняются.
                    time.sleep(.01 if "files-cancel-start" in desktop_sent and "files-cancel" not in desktop_sent else .2)
                else: raise RuntimeError("installed Bundle acceptance timed out; see serial.log")
                expected = ["installed independent directory Bundles", "wrong UID denied", "same UID outside session denied", "ungranted content denied without data creation", "credentials isolation persistence and exit status verified", "signal status verified", "installed Luna CLI launched", "dynamic musl closure and missing-library grant denial verified", "external named and abstract UNIX sockets denied", "setsid descendant reclaimed on leader exit", "logout descendants reclaimed and data/config persisted", "installed registry boots and data/config persists after reboot", "COMPLETE"]
                if os.environ.get("LUNA_CANONICAL_WAYLAND_FIXTURE"):
                    expected += ["Wayland capability denied without data creation", "real Weston Wayland window dynamic libraries and UID verified", "Wayland application reclaimed on logout"]
                    if "Wayland app exited before window" in text: raise RuntimeError("Wayland app exited before window")
                if (Path(os.environ["LUNA_CANONICAL_INPUT_ROOT"])/"desktop-bundles").is_dir():
                    for message in ["graphical Yazi navigation and folder creation verified", "GUI copy conflict rename move Trash recovery and recursive links verified", "GUI in-progress copy cancellation and staging cleanup verified", "Files menu window reclaimed on close", "interactive Ghostty fish Bash sh Yazi and private PTY verified", "terminal descendants reclaimed and home persists", "verified catalog filters grants and rejects stale pin before storage", "Noctalia menu launched registered Files", "Noctalia menu launched registered Terminal defaults", "Noctalia menu launched independent registered Weston fixture", "catalog wrong UID denied", "catalog outside session denied"]:
                        if "LUNA-DESKTOP: " + message not in text: raise RuntimeError("missing desktop acceptance: " + message)
                    verify_files_menu_exit(text)
                    (root/"desktop-results.txt").write_text("Verified catalog, stale-pin denial, peer denials and real Noctalia menu launch of Files/Terminal/independent Weston: PASS\nRegistered Files GUI/copy-conflict-rename-move-Trash-recovery-recursive-links/cancellation/clean-exit and interactive Ghostty/fish/private PTY/lifecycle: PASS\n")
                for message in expected:
                    if "LUNA-BUNDLES: " + message not in text: raise RuntimeError(f"missing acceptance: {message}")
                for failure in ("descendant survived", "wrong exit", "application broker unavailable", "application broker failed", "session descendant boundary unavailable", "cgroup cleanup failed", "Kernel panic", "Wayland app exited before window", "application child setup failed"):
                    if failure in text: raise RuntimeError(failure)
                if sent != 3 or text.count("application broker ready for UserSession") != 3: raise RuntimeError("expected three real authenticated sessions")
                (root / "results.txt").write_text("\n".join(message + " PASS" for message in expected) + "\nInstalled Bundle UserSession acceptance: PASS\n")
                print("Installed Bundle UserSession acceptance: PASS")
    finally:
        if qemu is not None:
            qemu.terminate()
            try: qemu.wait(timeout=3)
            except subprocess.TimeoutExpired: qemu.kill(); qemu.wait()
        if overlay.exists(): overlay.unlink()
        if artifacts.digest(image) != before: raise RuntimeError("source image changed")

if __name__ == "__main__": main()
