#!/usr/bin/env python3
"""Bounded development image helpers. Operates on regular files, never devices."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile

REPO = Path(__file__).resolve().parent.parent
BLOCK = 65536


def run(*args, cwd=None):
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, text=True, cwd=cwd).stdout


def regular(path):
    path = Path(path).absolute()
    if ".." in path.parts or path.resolve() != path or not stat.S_ISREG(path.stat().st_mode):
        raise ValueError(f"not a regular image/source file: {path}")
    return path


def partition(image, number):
    lines = run("sgdisk", "-i", number, regular(image)).splitlines()
    first = int(next(line.split(":", 1)[1].split()[0] for line in lines if line.startswith("First sector:")))
    last = int(next(line.split(":", 1)[1].split()[0] for line in lines if line.startswith("Last sector:")))
    offset, length = first * 512, (last - first + 1) * 512
    if offset < 512 or length <= 0 or offset + length > Path(image).stat().st_size:
        raise ValueError("partition outside image")
    return offset, length


def sparse_partition(image, number, destination):
    offset, length = partition(image, number)
    with regular(image).open("rb") as source, Path(destination).open("xb") as target:
        source.seek(offset)
        remaining = length
        while remaining:
            data = source.read(min(BLOCK, remaining))
            if not data:
                raise ValueError("short image read")
            if data != bytes(len(data)):
                target.write(data)
            else:
                target.seek(len(data), 1)
            remaining -= len(data)
        target.truncate(length)
    return offset, length


def quoted(path):
    value = str(path)
    if "\n" in value or "\r" in value or "\0" in value:
        raise ValueError("unsupported control character in path")
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def debug(fs, command, write=False):
    return run("debugfs", *(["-w"] if write else []), "-R", command, fs)


def system_version(sys_fs):
    entries = debug(sys_fs, "ls -p /images").splitlines()
    names = [line.split("/")[5] for line in entries if len(line.split("/")) > 5]
    names = [name for name in names if name.startswith("luna-") and name.endswith(".squashfs")]
    if len(names) != 1:
        raise ValueError("development patch requires exactly one selected System Image")
    return names[0]


def extract(image, work):
    work = Path(work)
    parts = work / "parts"
    parts.mkdir(parents=True, exist_ok=False)
    for number, name in [(1, "efi.fs"), (2, "luna-sys.fs"), (3, "luna-data.fs")]:
        sparse_partition(image, number, parts / name)
    name = system_version(parts / "luna-sys.fs")
    debug(parts / "luna-sys.fs", f"dump /images/{name} {quoted(work / name)}")
    recovery = work / "luna-sys/recovery"
    recovery.mkdir(parents=True)
    debug(parts / "luna-sys.fs", f"dump /recovery/recovery.squashfs {quoted(recovery / 'recovery.squashfs')}")
    regular(work / name)
    regular(recovery / "recovery.squashfs")


def create_overlay(image, target):
    image = regular(image)
    target = Path(target).absolute()
    if ".." in target.parts or target.resolve() != target:
        raise ValueError("overlay path must be canonical without symlinks")
    if target.exists() or target.is_symlink():
        raise ValueError(f"overlay already exists: {target}")
    # qemu-img requires an explicit backing format and an absolute backing path.
    run("qemu-img", "create", "-f", "qcow2", "-F", "raw", "-b", image, target)
    return target


def write_overlay_block(overlay, offset, data, work):
    (work / "chunk").write_bytes(data)
    # qemu-io command parsing treats double quotes as filename bytes. Use an
    # internal basename and cwd instead of embedding a host path in -c.
    run("qemu-io", "-f", "qcow2", "-c", f"write -q -s chunk {offset} {len(data)}", regular(overlay), cwd=work)


def lifecycle_session(text, action):
    if action not in ("logout", "reboot", "shutdown"):
        raise ValueError("unknown lifecycle action")
    marker = '    echo "luna-graphical-session: starting Niri" >&2\n'
    if text.count(marker) != 1:
        raise ValueError("graphical session action marker mismatch")
    # The marker is inside dbus-run-session's single-quoted sh -c program.
    # The request must contain no single quotes of its own.
    request = r'''echo "LUNA-LIFECYCLE: planned ACTION through Noctalia"
PERSISTENT_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
if [ -e "$HOME/.cache/luna-lifecycle-action" ]; then
    if [ "$(cat "$HOME/luna-persistence.txt")" != "persisted-ACTION" ] ||
       [ "$(cat "$PERSISTENT_CONFIG/luna-persistence.txt")" != "persisted-ACTION" ]; then
        echo "LUNA-LIFECYCLE: persistent data LOST"
        exit 1
    fi
    echo "LUNA-LIFECYCLE: persistent home and config verified"
else
    mkdir -p "$PERSISTENT_CONFIG"
    printf "%s\n" "persisted-ACTION" > "$HOME/luna-persistence.txt"
    printf "%s\n" "persisted-ACTION" > "$PERSISTENT_CONFIG/luna-persistence.txt"
    echo "LUNA-LIFECYCLE: persistent home and config written"
fi
(
    sleep 30
    if [ ! -e "$HOME/.cache/luna-lifecycle-action" ]; then
        mkdir -p "$HOME/.cache"
        touch "$HOME/.cache/luna-lifecycle-action"
        # This process predates Niri, so it cannot inherit WAYLAND_DISPLAY.
        # Noctalia IPC uses noctalia-$WAYLAND_DISPLAY.sock; select the live pair.
        found_display=""
        for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
            display="${socket##*/}"
            if [ -S "$socket" ] && [ -S "$XDG_RUNTIME_DIR/noctalia-$display.sock" ]; then
                found_display="$display"
                break
            fi
        done
        [ -n "$found_display" ] || { echo "LUNA-LIFECYCLE: Noctalia IPC not ready" >&2; exit 1; }
        export WAYLAND_DISPLAY="$found_display"
        echo "LUNA-LIFECYCLE: requesting ACTION through Noctalia"
        /usr/bin/noctalia msg session ACTION
    fi
) &
'''.replace("ACTION", action)
    return text.replace(marker, request + marker, 1)


def patch_system_image(image, replacements, *, overlay=None, scratch=None, session_action=None, workbench_lock=None, prepare_test_tree=None):
    if prepare_test_tree is not None and overlay is None:
        raise ValueError("test fixture preparation requires a disposable overlay")
    image = regular(image)
    if overlay is None:
        # In-place writes are restricted to the managed experimental slot.
        root = REPO / "target/canonical-workbench"
        if root.resolve() != root or workbench_lock is None:
            raise ValueError("in-place patch requires the locked workbench controller")
        lock_stat = (root / ".lock").stat()
        descriptor_stat = os.fstat(workbench_lock.fileno())
        if (lock_stat.st_dev, lock_stat.st_ino) != (descriptor_stat.st_dev, descriptor_stat.st_ino):
            raise ValueError("patch lock does not belong to this workbench")
        fcntl.flock(workbench_lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        state = json.loads((root / "state.json").read_text())
        if state.get("format") != 1 or {state.get("reference"), state.get("experiment")} != {"a", "b"}:
            raise ValueError("invalid workbench state")
        expected = root / "slots" / state["experiment"] / "luna-0.1.0.img"
        if image != expected or image.is_symlink():
            raise ValueError("in-place patch is only allowed in the managed experimental slot")
    else:
        overlay = create_overlay(image, overlay)
    scratch = Path(scratch) if scratch else REPO / "target"
    with tempfile.TemporaryDirectory(prefix="luna-patch-", dir=scratch) as temporary:
        work = Path(temporary)
        sys_fs = work / "sys.fs"
        offset, length = sparse_partition(image, 2, sys_fs)
        name = system_version(sys_fs)
        original = work / "original.squashfs"
        debug(sys_fs, f"dump /images/{name} {quoted(original)}")
        tree = work / "system-image"
        run("unsquashfs", "-no-progress", "-d", tree, original)
        if prepare_test_tree is not None:
            prepare_test_tree(tree)
        if session_action:
            session = tree / "apps/luna-graphical-session/luna-graphical-session"
            session.write_text(lifecycle_session(session.read_text(), session_action))
            run("sh", "-n", session)
        replaced = []
        for logical, source in replacements:
            relative = Path(logical)
            if relative.is_absolute() or not relative.parts or any(p in (".", "..") for p in relative.parts) or relative.parts[0] not in ("apps", "config", "resources", "libs", "drivers", "firmware"):
                raise ValueError("replacement must be a normalized System Image resource")
            if str(relative) != logical:
                raise ValueError("replacement path is not canonical")
            target = tree / relative
            for component in [target, *target.parents]:
                if component == tree:
                    break
                if component.is_symlink():
                    raise ValueError("symlink in System Image replacement")
            regular(target)
            mode = stat.S_IMODE(target.stat().st_mode)
            shutil.copyfile(regular(source), target)
            target.chmod(mode)
            replaced.append({"resource": logical, "source": str(regular(source)), "sha256": digest(target)})
        updated = work / "updated.squashfs"
        run("mksquashfs", tree, updated, "-noappend", "-comp", "zstd", "-all-root", "-no-xattrs", "-processors", "2", "-no-progress")
        debug(sys_fs, f"rm /images/{name}", write=True)
        debug(sys_fs, f"write {quoted(updated)} /images/{name}", write=True)
        debug(sys_fs, f"sif /images/{name} uid 0", write=True)
        debug(sys_fs, f"sif /images/{name} gid 0", write=True)
        debug(sys_fs, f"sif /images/{name} mode 0100644", write=True)
        verified = work / "verified.squashfs"
        debug(sys_fs, f"dump /images/{name} {quoted(verified)}")
        if digest(updated) != digest(verified):
            raise ValueError("System Image write verification failed")
        check = subprocess.run(["e2fsck", "-fn", str(sys_fs)], capture_output=True, text=True)
        if check.returncode != 0:
            raise ValueError(f"updated LUNA-SYS failed e2fsck: {check.stdout} {check.stderr}")
        changed = 0
        with image.open("rb") as before, sys_fs.open("rb") as after:
            before.seek(offset)
            destination = None if overlay else image.open("r+b")
            try:
                for position in range(0, length, BLOCK):
                    old, new = before.read(BLOCK), after.read(BLOCK)
                    if old != new:
                        if overlay:
                            write_overlay_block(overlay, offset + position, new, work)
                        else:
                            destination.seek(offset + position)
                            destination.write(new)
                        changed += len(new)
                if destination:
                    destination.flush()
                    os.fsync(destination.fileno())
            finally:
                if destination:
                    destination.close()
        return {"changed_bytes": changed, "system_image": name, "system_image_sha256": digest(updated), "replacements": replaced}


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def cleanup_build(output):
    output = Path(output).absolute()
    if ".." in output.parts or output.resolve() != output:
        raise ValueError("cleanup path must be canonical without navigation")
    # Never recursively remove an arbitrary caller directory. Only known build
    # subtrees below the repository's target, after tests have finished.
    if not output.is_relative_to(REPO / "target"):
        raise ValueError("automatic cleanup is limited to repository target")
    for parent in [output, *output.parents]:
        if parent == REPO:
            break
        if parent.is_symlink():
            raise ValueError("symlink in cleanup root")
    marker = output / ".luna-canonical-output-v1"
    if marker.is_symlink() or not marker.is_file() or marker.read_text() != "Project Luna generated canonical output v1\n":
        raise ValueError("refusing cleanup of an unmarked output directory")
    for name in (".build", "luna-sys", "luna-data"):
        path = output / name
        if path.is_symlink():
            raise ValueError("symlink in build artifact")
        if path.exists():
            # App Manager intentionally makes installed package directories
            # immutable. Re-enable owner traversal/unlink only in this marked
            # generated build subtree, never through its symlink aliases.
            for _, _, _, descriptor in os.fwalk(path, follow_symlinks=False):
                os.fchmod(descriptor, stat.S_IMODE(os.fstat(descriptor).st_mode) | 0o700)
            shutil.rmtree(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("overlay"); p.add_argument("image"); p.add_argument("output")
    p = commands.add_parser("extract"); p.add_argument("image"); p.add_argument("work")
    p = commands.add_parser("cleanup-build"); p.add_argument("output")
    p = commands.add_parser("patch-system-image")
    p.add_argument("image"); p.add_argument("--overlay"); p.add_argument("--scratch")
    p.add_argument("--replace", nargs=2, action="append", default=[], metavar=("LOGICAL", "SOURCE"))
    p.add_argument("--session-action", choices=("logout", "reboot", "shutdown"))
    args = parser.parse_args()
    if args.command == "overlay": create_overlay(args.image, args.output)
    elif args.command == "extract": extract(args.image, args.work)
    elif args.command == "cleanup-build": cleanup_build(args.output)
    else:
        if not args.replace and not args.session_action: parser.error("patch requires a resource replacement or session action")
        print(json.dumps(patch_system_image(args.image, args.replace, overlay=args.overlay, scratch=args.scratch, session_action=args.session_action)))


if __name__ == "__main__":
    main()
