#!/usr/bin/env python3
"""Stage the cached Alpine Niri runtime closure into a temporary LUNA-DATA tree.

This is an experiment-only adapter: Niri, seatd, eudev and their runtime
libraries remain DATA providers, while Luna owns boot, authentication and
process supervision.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GRAPHICS_BUILD_ROOT = Path(
    os.environ.get(
        "LUNA_ALPINE_GRAPHICS_BUILD_ROOT",
        str(ROOT / "dist/.build/alpine-foundation-graphics"),
    )
)
MESA_VIRGL_OUT = GRAPHICS_BUILD_ROOT / "mesa/out"
NOCTALIA_OUT = GRAPHICS_BUILD_ROOT / "noctalia/out"
NOCTALIA_PACKAGES_FILE = ROOT / "tools/alpine-foundation/noctalia-runtime-packages.txt"
CACHES = [
    ROOT / "dist/.build/alpine-niri-apk-cache",
    GRAPHICS_BUILD_ROOT / "noctalia-runtime-apks",
    ROOT / "dist/.build/noctalia-runtime-apks",
    ROOT / "dist/.build/alpine-greeter-apks/packages",
    ROOT / "dist/.build/udev-provider-root/repo",
]
REQUIRED_PACKAGES = (
    "niri",
    "mesa-egl",
    "mesa-gbm",
    "mesa-dri-gallium",
    "busybox",
    "seatd",
    "eudev",
    "libinput",
    "xkeyboard-config",
    # Niri's Rust Wayland backend dlopens the server library. This dependency
    # is not represented in the APK's regular ELF NEEDED list, so require it.
    "wayland-libs-server",
    "fontconfig",
    "adwaita-fonts-sans",
)
EXCLUDED = {".PKGINFO", ".INSTALL", ".trigger"}


def package_metadata(path: Path) -> tuple[str, list[str], list[str]]:
    with tarfile.open(path, "r:gz") as archive:
        member = archive.extractfile(".PKGINFO")
        if member is None:
            raise ValueError(f"{path} does not contain .PKGINFO")
        lines = member.read().decode(errors="replace").splitlines()
    name = next(
        (line.split("=", 1)[1].strip() for line in lines if line.startswith("pkgname = ")),
        "",
    )
    deps = [line.split("=", 1)[1].strip() for line in lines if line.startswith("depend = ")]
    provides = [line.split("=", 1)[1].strip() for line in lines if line.startswith("provides = ")]
    if not name:
        raise ValueError(f"{path} has no package name")
    return name, deps, provides


def index_packages() -> tuple[dict[str, dict], dict[str, dict], dict[str, dict], dict[str, dict]]:
    packages: dict[str, dict] = {}
    package_provider: dict[str, dict] = {}
    soname_provider: dict[str, dict] = {}
    command_provider: dict[str, dict] = {}
    for cache in CACHES:
        if not cache.is_dir():
            continue
        for path in sorted(cache.glob("*.apk")):
            try:
                name, deps, provides = package_metadata(path)
            except (OSError, tarfile.TarError, ValueError):
                continue
            package = {"name": name, "path": path, "deps": deps}
            # Cache order gives preferred providers: the Niri closure first,
            # then Noctalia, greeter and finally udev support packages.
            packages.setdefault(name, package)
            for item in provides:
                if item.startswith("so:"):
                    soname = item[3:].split("=", 1)[0]
                    soname_provider.setdefault(soname, package)
                elif item.startswith("cmd:"):
                    command = item[4:].split("=", 1)[0]
                    command_provider.setdefault(command, package)
                elif not item.startswith(("pc:", "trigger:")):
                    virtual_package = item.split("=", 1)[0]
                    package_provider.setdefault(virtual_package, package)
    return packages, package_provider, soname_provider, command_provider


def dependency_closure(include_noctalia: bool = False) -> list[dict]:
    packages, package_provider, soname_provider, command_provider = index_packages()
    selected: dict[str, dict] = {}
    unresolved: set[str] = set()
    pending = list(REQUIRED_PACKAGES)
    if include_noctalia:
        pending.extend(
            line.strip()
            for line in NOCTALIA_PACKAGES_FILE.read_text().splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        )
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        package = packages.get(name)
        if package is None:
            unresolved.add(f"package:{name}")
            continue
        selected[name] = package
        for dep in package["deps"]:
            if dep.startswith("so:"):
                soname = dep[3:].split("=", 1)[0]
                provider = soname_provider.get(soname)
                if provider:
                    pending.append(provider["name"])
                else:
                    unresolved.add(f"soname:{soname} required by {name}")
            elif dep.startswith("cmd:"):
                command = dep[4:].split("=", 1)[0]
                provider = command_provider.get(command)
                if provider:
                    pending.append(provider["name"])
                else:
                    unresolved.add(f"command:{command} required by {name}")
            elif dep.startswith(("!", "/")) or dep.startswith(("pc:", "trigger:")):
                continue
            else:
                # APK runtime dependencies are package names with an optional
                # version relation. Ignore virtual base packages only when the
                # cache has no concrete package archive for them.
                dependency = dep.split("=", 1)[0]
                if dependency in packages:
                    pending.append(dependency)
                elif dependency in package_provider:
                    pending.append(package_provider[dependency]["name"])
                elif dependency not in {"musl", "busybox"}:
                    unresolved.add(f"dependency:{dependency} required by {name}")

    if unresolved:
        raise RuntimeError("unresolved desktop test package dependencies:\n  " + "\n  ".join(sorted(unresolved)))
    return sorted(selected.values(), key=lambda item: item["name"])


def extract_package(path: Path, destination: Path) -> None:
    # APK payloads are fetched from the repository's local cache. Use tar's
    # normal link-aware extraction, while excluding package manager metadata.
    result = subprocess.run(
        ["tar", "-xzf", str(path), "-C", str(destination),
         "--exclude=.PKGINFO", "--exclude=.SIGN.*", "--exclude=.INSTALL", "--exclude=.trigger"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    if result.returncode:
        raise RuntimeError(f"could not extract {path.name}: {result.stderr.strip()}")


def copy_tree(source: Path, target: Path) -> None:
    if source.is_dir():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, target, symlinks=True, dirs_exist_ok=True)


def install_file(source: Path, target: Path) -> None:
    if not source.is_file():
        raise FileNotFoundError(f"cached runtime file not found: {source}")
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    target.chmod(target.stat().st_mode | 0o111)


def overlay_tree(source: Path, target: Path) -> None:
    """Overlay files and symlinks, replacing pre-existing destinations."""
    target.mkdir(parents=True, exist_ok=True)
    for entry in source.iterdir():
        dest = target / entry.name
        if entry.is_dir() and not entry.is_symlink():
            if dest.is_symlink() or (dest.exists() and not dest.is_dir()):
                dest.unlink()
            overlay_tree(entry, dest)
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        if dest.is_symlink() or (dest.exists() and not dest.is_dir()):
            dest.unlink()
        elif dest.is_dir():
            shutil.rmtree(dest)
        if entry.is_symlink():
            dest.symlink_to(entry.readlink())
        else:
            shutil.copy2(entry, dest)


def main() -> int:
    if len(sys.argv) not in {2, 3} or (len(sys.argv) == 3 and sys.argv[2] != "--with-noctalia"):
        print(
            f"usage: {Path(sys.argv[0]).name} EXTRA_DATA_ROOT [--with-noctalia]",
            file=sys.stderr,
        )
        return 2
    include_noctalia = len(sys.argv) == 3
    # This helper creates disposable test content and the caller later removes
    # EXTRA_DATA_ROOT. Never accept an existing directory (or symlink) here:
    # an accidental environment override must not erase user data.
    extra_data = Path(sys.argv[1]).absolute()
    temp_root = Path(tempfile.gettempdir()).resolve()
    if (
        extra_data.parent != temp_root
        or not extra_data.name.startswith("luna-niri-provider-")
        or extra_data.is_symlink()
    ):
        print(
            "Niri test staging path must be a direct, non-symlink child of "
            f"{temp_root} named luna-niri-provider-*",
            file=sys.stderr,
        )
        return 2
    if extra_data.exists():
        print(f"Niri test staging path already exists: {extra_data}", file=sys.stderr)
        return 2

    noctalia_binary = NOCTALIA_OUT / "usr/local/bin/noctalia"
    noctalia_share = NOCTALIA_OUT / "usr/local/share/noctalia"
    if include_noctalia and (not noctalia_binary.is_file() or not (noctalia_share / "assets").is_dir()):
        print(
            "Noctalia test artifacts missing; run tools/build-alpine-foundation-graphics.sh: "
            f"{noctalia_binary} and {noctalia_share / 'assets'}",
            file=sys.stderr,
        )
        return 1

    work = extra_data / ".niri-package-root"
    work.mkdir(parents=True)
    try:
        packages = dependency_closure(include_noctalia)
        for package in packages:
            extract_package(package["path"], work)

        system = extra_data / "system"
        apps = system / "apps"
        libraries = system / "libs"
        niri_app = apps / "niri"
        udev_app = apps / "udevd"
        niri_resources = niri_app / "resources"

        install_file(work / "usr/bin/niri", niri_app / "niri")
        busybox = next(
            (path for path in (work / "bin/busybox", work / "usr/bin/busybox") if path.is_file()),
            None,
        )
        if busybox is None:
            raise FileNotFoundError("BusyBox package did not provide its multi-call binary")
        install_file(busybox, apps / "busybox" / "busybox")
        install_file(work / "usr/bin/seatd", apps / "seatd" / "seatd")

        udevd = next(
            (path for path in (work / "sbin/udevd", work / "usr/bin/udevd", work / "usr/sbin/udevd") if path.is_file()),
            None,
        )
        if udevd is None:
            raise FileNotFoundError("eudev package did not provide udevd")
        install_file(udevd, udev_app / "udevd")

        # Alpine's /sbin/udevadm is an absolute symlink to /bin/udevadm.
        # Resolving that symlink against the build host would accidentally
        # copy the host's systemd/glibc binary into the musl guest. Select the
        # regular package payload instead, and never copy a host-resolved link.
        udevadm = next(
            (
                path
                for path in (work / "bin/udevadm", work / "usr/bin/udevadm", work / "sbin/udevadm")
                if path.is_file() and not path.is_symlink()
            ),
            None,
        )
        if udevadm is not None:
            install_file(udevadm, apps / "udevadm" / "udevadm")

        if include_noctalia:
            noctalia_app = apps / "noctalia"
            install_file(noctalia_binary, noctalia_app / "noctalia")
            copy_tree(noctalia_share, noctalia_app / "resources" / "noctalia")

            dbus_app = apps / "dbus-daemon"
            install_file(work / "usr/bin/dbus-daemon", dbus_app / "dbus-daemon")
            install_file(work / "usr/bin/dbus-run-session", dbus_app / "dbus-run-session")
            copy_tree(work / "usr/share/dbus-1", dbus_app / "resources" / "dbus-1")

            wrapper = dbus_app.parent / "noctalia" / "luna-noctalia-test"
            wrapper.write_text(
                "#!/bin/sh\n"
                "set -eu\n"
                "export NOCTALIA_ASSETS_DIR=/usr/share/noctalia/assets\n"
                "export NOCTALIA_LOG_LEVEL=debug\n"
                "export XDG_DATA_DIRS=/usr/local/share:/usr/share\n"
                "echo 'LUNA-NOCTALIA: LAUNCHING /usr/bin/noctalia' "
                ">>\"${XDG_RUNTIME_DIR:-/tmp}/luna-noctalia.log\"\n"
                "exec /usr/bin/dbus-run-session -- /usr/bin/noctalia "
                ">>\"${XDG_RUNTIME_DIR:-/tmp}/luna-noctalia.log\" 2>&1\n"
            )
            wrapper.chmod(0o755)

        copy_tree(work / "lib", libraries)
        copy_tree(work / "usr/lib", libraries)

        # Alpine's packaged Mesa lacks the VirGL Gallium driver needed by a
        # QEMU virtio-gpu GL device. Reuse Luna's tracked, reproducibly built
        # mesa-virgl artifact (virgl + llvmpipe), overriding the package Mesa
        # libs and DRI modules in this isolated DATA provider.
        mesa_local_lib = MESA_VIRGL_OUT / "usr/local/lib"
        mesa_dri = MESA_VIRGL_OUT / "usr/lib/dri"
        mesa_required = [
            mesa_local_lib / "libgallium-26.1.6.so",
            mesa_local_lib / "libEGL.so.1.0.0",
            mesa_local_lib / "libgbm.so.1.0.0",
            mesa_dri / "virtio_gpu_dri.so",
        ]
        missing_mesa = [path for path in mesa_required if not path.exists()]
        if missing_mesa:
            raise FileNotFoundError(
                "Luna Mesa VirGL artifact missing; run tools/build-alpine-foundation-graphics.sh: "
                + ", ".join(str(path) for path in missing_mesa)
            )
        overlay_tree(mesa_local_lib, libraries)
        # Replace the generic Alpine DRI module directory rather than leave
        # stale symlinks for hardware drivers not included in the VirGL build.
        shutil.rmtree(libraries / "dri", ignore_errors=True)
        copy_tree(mesa_dri, libraries / "dri")

        copy_tree(work / "usr/lib/udev", udev_app / "resources" / "udev")
        copy_tree(work / "etc/udev", system / "config" / "udev")
        copy_tree(work / "etc/fonts", system / "config" / "fonts")
        copy_tree(work / "etc/libinput", system / "config" / "libinput")
        copy_tree(work / "usr/share/X11", niri_resources / "X11")
        copy_tree(work / "usr/share/xkeyboard-config-2", niri_resources / "xkeyboard-config-2")
        copy_tree(work / "usr/share/libinput", niri_resources / "libinput")
        copy_tree(work / "usr/share/glvnd", niri_resources / "glvnd")
        copy_tree(work / "usr/share/fonts", system / "resources" / "fonts")

        # Keep host-size package payloads out of the staged DATA tree after
        # copying only the runtime closure and the resources the compositor uses.
        shutil.rmtree(work)
        print(f"Staged Niri Wayland compositor from {len(packages)} cached Alpine packages")
        print(f"Extra test DATA root: {extra_data}")
        return 0
    except Exception as error:
        shutil.rmtree(work, ignore_errors=True)
        print(f"Niri test staging failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
