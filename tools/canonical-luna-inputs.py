#!/usr/bin/env python3
"""Record or verify the complete canonical build input snapshot, including links."""
import argparse
import hashlib
import json
import os
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
RECIPES = [
    "tools/prepare-canonical-luna-inputs.sh",
    "tools/canonical-luna-inputs.py",
    "tools/build-alpine-foundation-graphics.sh",
    "tools/build-nerd-fonts.sh",
    "tools/alpine-foundation/mesa-virgl.Dockerfile",
    "tools/alpine-foundation/noctalia.Dockerfile",
    "tools/alpine-foundation/canonical-provider-packages.txt",
    "tools/alpine-foundation/desktop-service-packages.txt",
    "tools/alpine-foundation/noctalia-runtime-packages.txt",
]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(root):
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            key = path.relative_to(root).as_posix()
            if key in ("inputs.lock.json", "inputs.lock.tmp"):
                continue
            if path.is_symlink():
                result[key] = {"link": os.readlink(path)}
            elif path.is_file():
                result[key] = {"sha256": digest(path), "mode": path.stat().st_mode & 0o777}
            elif path.is_dir():
                result[key] = {"directory": True}
            else:
                raise SystemExit(f"unsupported input node: {path}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("record", "verify"))
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    if not root.is_dir():
        raise SystemExit(f"input snapshot missing: {root}")
    required = [
        "alpine-runtime-rootfs.tar.gz", "kernel/bzImage", "niri-apks",
        "provider-apks", "provider-root/usr/lib", "graphics/mesa/out/usr/local/lib",
        "graphics/noctalia/out/usr/local/bin/noctalia",
        "graphics/noctalia-runtime-apks",
        "fonts/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf",
    ]
    for name in required:
        if not (root / name).exists():
            raise SystemExit(f"required build input missing: {root / name}")
    current = {
        "format": 1,
        "recipes": {name: digest(REPO / name) for name in RECIPES},
        "inputs": inventory(root),
    }
    lock = root / "inputs.lock.json"
    if args.action == "record":
        temporary = root / "inputs.lock.tmp"
        temporary.write_text(json.dumps(current, indent=2, sort_keys=True) + "\n")
        temporary.replace(lock)
    else:
        expected = json.loads(lock.read_text())
        for section in ("format", "recipes", "inputs"):
            if current[section] != expected[section]:
                if isinstance(current[section], dict):
                    keys = sorted(set(current[section]) | set(expected[section]))
                    changed = [k for k in keys if current[section].get(k) != expected[section].get(k)]
                    raise SystemExit(f"canonical input {section} changed: {', '.join(changed[:12])}")
                raise SystemExit(f"canonical input {section} changed")
    print(f"Canonical input snapshot {args.action}: PASS ({len(current['inputs'])} nodes)")


if __name__ == "__main__":
    main()
