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
luna-init
 ↓
SYSTEM ext4 + selected SquashFS immutable source
 ↓
RAM-backed logical /
 ↓
DATA ext4
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
interactive shell / graphical session
```

Luna uses initramfs as early userspace, but the final system root is not a
persistent filesystem on disk. `luna-init` constructs a RAM-backed logical root
from controlled sources and then starts `luna-system-runtime`.

The selected System Image is mounted only as an internal immutable source while
boot-critical resources are materialized. The architecture does not use a
classic `switch_root` to make the SquashFS itself the final `/`.

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
- a static x86_64 BusyBox binary with `mount`, `sh` and the normal early-userspace
  filesystem utilities enabled.

A static BusyBox is useful here because it can provide the small set of early
userspace utilities without requiring a separate libc tree. It is an early
bootstrap dependency, not the final system init architecture.

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

The DATA partition remains physically separate from SYSTEM. The runtime view of
DATA is assembled into the logical root according to Luna's mapping/runtime
rules; physical SYSTEM/DATA paths are not the application-facing contract.

## Current early userspace

The dedicated `components/luna-init` implementation is the canonical early
userspace boundary. Its target behavior is:

1. prepare `/proc`, `/sys`, `/dev`, `/run` and `/tmp` runtime filesystems;
2. discover SYSTEM and DATA from `/proc/cmdline` / boot context;
3. validate and mount SYSTEM read-only;
4. locate and validate the selected SquashFS System Image;
5. mount that image at an internal source location rather than making it the
   final `/`;
6. create the RAM-backed logical root;
7. materialize the boot-critical system base into RAM;
8. make DATA and other approved resources available through controlled runtime
   mappings;
9. start `luna-system-runtime` as the normal userspace supervisor/PID 1.

Additional immutable resources may be hydrated lazily after the initial base is
ready. The exact boot-critical manifest/dependency closure and fully independent
lazy hydration mechanism are still implementation work governed by the accepted
RAM-hydration decision.

## Expected result

After the kernel and boot messages, `luna-system-runtime` starts and creates the
initial `UserSession`. The QEMU terminal should therefore become the configured
Luna development shell or graphical session path, depending on the test image.

## Important limitation

The current application launcher uses Unix `CommandExt::pre_exec` for namespace
setup. Rust documents that this hook executes after `fork` in a constrained
post-fork environment and warns against complex non-async-signal-safe work there.
It is acceptable for this single-process bring-up prototype, but it is **not**
the final production process-launch mechanism.

Application isolation does not use a separate `luna-app-init` component. The
system-wide `luna-system-runtime` remains PID 1, while `luna-app-runtime` owns
ApplicationInstance lifecycle and launches the application directly through the
namespace/materialization boundary. A PID namespace is not required by the
default application-isolation model.

The next hardening step is a dedicated Linux child-creation/process setup
primitive for the namespace operations that remain necessary, without creating
an additional application init layer.
