#!/usr/bin/env python3
"""Stage a pinned real Wayland test Bundle from verified canonical inputs.

The APK signature is verified separately with `apk verify`; this tool pins its
exact bytes and extracts only the chosen executable. It never fetches packages.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

APK_NAME = "weston-clients-14.0.2-r5.apk"
APK_SHA256 = "9b8ee28e3217c281244691c9496e86cc3eb1a4bbcd94c3109b8cc0fad0a5ccc1"
REPO = Path(__file__).resolve().parent.parent

def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    inputs, apk, output = args.inputs.resolve(), args.apk.resolve(), args.output.absolute()
    subprocess.run(["python3", str(REPO / "tools/canonical-luna-inputs.py"), "verify", str(inputs)], check=True)
    if apk.name != APK_NAME or digest(apk) != APK_SHA256:
        raise SystemExit("unexpected Weston APK content")
    if output.exists() or output.is_symlink():
        raise SystemExit("fixture output already exists")
    output.mkdir(parents=True)
    payload = output / "payload"
    (payload / "bin").mkdir(parents=True)
    (payload / "lib").mkdir()
    with tarfile.open(apk, "r:gz") as archive:
        member = archive.getmember("usr/bin/weston-simple-shm")
        if not member.isfile() or member.size > 1024 * 1024:
            raise SystemExit("unexpected Weston executable")
        with archive.extractfile(member) as stream, (payload / "bin/weston-simple-shm").open("wb") as target:
            shutil.copyfileobj(stream, target)
    resources = {
        "lib/ld-musl-x86_64.so.1": "lib/ld-musl-x86_64.so.1",
        "lib/libc.musl-x86_64.so.1": "lib/ld-musl-x86_64.so.1",
        "lib/libwayland-client.so.0": "usr/lib/libwayland-client.so.0",
        "lib/libffi.so.8": "usr/lib/libffi.so.8",
    }
    provenance = {"bin/weston-simple-shm": {"apk": APK_NAME, "apk_sha256": APK_SHA256}}
    provider = inputs / "provider-root"
    for destination, source in resources.items():
        selected = (provider / source).resolve(strict=True)
        if not selected.is_relative_to(provider) or not selected.is_file():
            raise SystemExit("library escapes verified provider input")
        shutil.copyfile(selected, payload / destination)
        provenance[destination] = {"input": str(selected.relative_to(inputs)), "sha256": digest(selected)}
    manifest = '''format = 1
[bundle]
id = "org.luna.weston-simple-shm"
name = "Weston Simple SHM"
version = "14.0.2"
type = "application"
[platform]
arch = "x86_64"
[entry]
exec = "bin/weston-simple-shm"
logical = "/bin/weston-simple-shm"
[capabilities]
requested = ["desktop.wayland"]
'''
    for resource in ["bin/weston-simple-shm", *resources]:
        (payload / resource).chmod(0o755)
        provenance[resource]["payload_sha256"] = digest(payload / resource)
        manifest += f'\n[[mapping]]\nlogical = "/{resource}"\nsource = "{resource}"\naccess = ["read", "execute"]\n'
    (output / "manifest.toml").write_text(manifest)
    (output / "provenance.json").write_text(json.dumps({"format": 1, "input_lock_sha256": digest(inputs / "inputs.lock.json"), "resources": provenance}, indent=2) + "\n")
    print(output)

if __name__ == "__main__":
    main()
