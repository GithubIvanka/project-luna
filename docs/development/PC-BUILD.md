# Project Luna x86_64 PC development build

This is the current reproducible bare-metal development image for Project Luna.
It produces one UEFI/GPT disk image and does not modify the build host disk.

## Result

`tools/build-pc-image.sh` creates:

```text
dist/luna-pc.img
```

The disk contains:

```text
EFI     128 MiB
SYSTEM  384 MiB
DATA    512 MiB
```

The image uses the accepted Luna physical storage model:

```text
EFI
SYSTEM
DATA
```

SWAP is intentionally omitted from the development image. A real installation
may add optional SWAP/ZRAM later.

## Boot chain

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
early initramfs
  ↓
SYSTEM (read-only)
  ↓
versioned SquashFS System Image
  ↓
DATA (read-write)
  ↓
switch_root
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
/usr/bin/luna-session
```

The default development session falls back to `/bin/sh` when a graphical
`niri-session` is not present. This means the image is currently usable as a
console development OS even before the complete graphical desktop payload is
packaged.

## Host prerequisites

On a Debian/Ubuntu-like build host install:

```bash
sudo apt install \
  busybox-static \
  cpio \
  dosfstools \
  e2fsprogs \
  gdisk \
  linux-image-amd64 \
  musl-tools \
  mtools \
  squashfs-tools
```

Rust stable and `rustup` are required. The builder installs the
`x86_64-unknown-linux-musl` and UEFI targets when needed.

The builder automatically looks for:

```text
/boot/vmlinuz-*
/usr/bin/busybox
/bin/busybox
```

Explicit paths may be provided with `LUNA_TEST_KERNEL` and `BUSYBOX`.

## Build

From the repository root:

```bash
tools/build-pc-image.sh
```

Optional version override:

```bash
LUNA_VERSION=0.1.0 tools/build-pc-image.sh
```

Optional prepared desktop root:

```bash
LUNA_DESKTOP_ROOT=/path/to/prepared-root tools/build-pc-image.sh
```

A prepared desktop root should contain the required Wayland/niri/Noctalia
payload. When `/usr/bin/niri-session` exists the Luna session launcher uses it.
The prepared root may also provide `/etc/luna/mode` containing `graphical` to
select the graphical `UserSession` launch boundary.

## Install to a real PC disk

The installer is deliberately destructive and requires both `--yes` and a
second textual confirmation. It refuses a target disk that has mounted
filesystems.

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/nvme0n1 --yes
```

Replace the target with the actual whole-disk device. Do not use a partition
such as `/dev/nvme0n1p1`.

Then reboot the PC in UEFI mode and select the Luna disk. The normal boot target
is the quiet PC path. Press `B` during boot for the existing Luna boot menu;
the serial development target remains available there.

## Current limitations

This is a development build, not a production installer.

The image still depends on a Linux kernel supplied by the build host or by the
CI image toolchain. The final production kernel inventory/compatibility and
persistent boot-success state remain separate architecture work.

The Linux application launcher still uses the development `pre_exec` child
setup path. Production hardening requires a dedicated child-creation primitive.

The full graphical payload is intentionally pluggable at this stage. The Luna
runtime/session architecture is present, but packaging the final niri + Noctalia
runtime tree and device/portal integration is still a later integration step.

## CI artifact

The repository CI contains a dedicated `Luna PC image` workflow. It builds the
same development disk on Ubuntu, verifies the generated files, prints their
SHA-256 values, and uploads `luna-pc-x86_64` as a workflow artifact.
