# QEMU/OVMF bootable development path

This document describes the current development bring-up path for running the
actual Luna userspace in QEMU. It is not a production installer.

## What the path tests

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux bzImage
 ↓
CPIO/gzip early userspace
 ↓
SYSTEM ext4
 ↓
selected SquashFS System Image
 ↓
DATA ext4
 ↓
switch_root
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
interactive shell
```

Linux initramfs is explicitly intended to provide a first userspace that mounts
the eventual root filesystem and then switches to it; this is the two-stage
model used by the current Luna handoff.

## Host requirements

Install:

- Rust stable;
- QEMU x86_64;
- OVMF;
- `sgdisk`;
- `mkfs.ext4` and `mkfs.fat`;
- `mtools` (`mcopy`, `mmd`);
- `mksquashfs`;
- `cpio` and `gzip`;
- a static x86_64 BusyBox binary with `mount`, `switch_root`, `sh` and the
  normal filesystem utilities enabled.

A static BusyBox is useful here because it can provide the small set of early
userspace utilities without requiring a separate libc tree. BusyBox provides a
`switch_root` applet specifically for replacing the initramfs root with the new
root.

## Environment

```bash
export OVMF_CODE=/path/to/OVMF_CODE.fd
export OVMF_VARS=/path/to/writable/OVMF_VARS.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export BUSYBOX=/path/to/static/x86_64/busybox
```

`OVMF_VARS` must be a writable copy. Do not point the test directly at a shared
firmware variables file.

## Run

From the repository root:

```bash
boot/luna-boot/tests/ovmf/build-and-run.sh
```

The builder prefers `x86_64-unknown-linux-musl` for `luna-system-runtime` when
that target is already installed. Otherwise it uses the host Linux target and
copies the required dynamic loader/libraries into the test System Image.

The Rust musl target is a supported Rust target and is statically linked by
default, which makes it appropriate for a small early/development image.

## Disk layout

The test disk contains:

```text
EFI    64 MiB
SYSTEM 256 MiB
DATA   128 MiB
```

The SYSTEM partition contains:

```text
kernels/test/bzImage
kernels/test/initramfs.img
images/luna-test.squashfs
```

The DATA partition is mounted as `/data` after the selected System Image is
mounted. The test image therefore exercises the same physical SYSTEM/DATA
separation used by the architecture.

## Current early userspace

`boot/luna-boot/tests/ovmf/luna-init` is deliberately a small shell-based
bring-up implementation. It:

1. mounts `/proc`, `/sys` and `/dev`;
2. mounts SYSTEM read-only;
3. reads `luna.system_image=` from `/proc/cmdline`;
4. loop-mounts the selected SquashFS image read-only;
5. mounts DATA at `/newroot/data`;
6. moves the early-userspace virtual filesystems into the new root;
7. uses BusyBox `switch_root` to execute `/sbin/init` from the System Image.

The final dedicated `luna-init` implementation remains a later hardening step;
the current script exists so the whole architecture can be exercised now.

## Expected result

After the kernel and boot messages, `luna-system-runtime` starts, creates the
initial `UserSession`, and launches `/bin/sh` with inherited serial stdio.
The QEMU terminal should therefore become an interactive Luna development shell.

## Important limitation

The current application launcher uses Unix `CommandExt::pre_exec` for namespace
setup. Rust documents that this hook executes after `fork` in a constrained
post-fork environment and warns against complex non-async-signal-safe work there.
It is acceptable for this single-process bring-up prototype, but it is **not**
the final production process-launch mechanism.

The next hardening step is a dedicated Linux child-creation primitive that can
create the required namespace before normal runtime execution without relying
on complex post-fork Rust operations.
