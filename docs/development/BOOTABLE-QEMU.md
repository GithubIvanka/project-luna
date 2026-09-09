# QEMU/OVMF bootable development path

This document describes the current development bring-up path for validating
the real Luna boot boundary in QEMU. It is not a production installer.

## What the path tests

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux bzImage
 ↓
luna-init ELF (loaded by luna-boot, executed directly by kernel)
 ↓
luna-init (PID 1)
 ↓
Luna system runtime (next milestone)
```

The first bring-up intentionally stops after proving that the kernel can accept
the Luna boot handoff, create FD 3, execute the selected `luna-init` ELF and keep
that process alive as PID 1.

There is no early userspace archive, no BusyBox bootstrap and no root-switching
stage in this path.

## Host requirements

Install or provide:

- Rust stable;
- QEMU x86_64;
- OVMF;
- `sgdisk`;
- `mkfs.ext4` and `mkfs.fat`;
- `mtools` (`mcopy`, `mmd`);
- `mksquashfs`.

## Environment

```bash
export OVMF_CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
export OVMF_VARS=/path/to/writable/OVMF_VARS_4M.fd
export LUNA_TEST_KERNEL=/home/sibitti/source/project-luna/dist/kernel/7.2.4/bzImage
```

`OVMF_VARS` must be a writable copy. Do not point the test directly at a shared
firmware variables file.

## Run

From the repository root:

```bash
boot/luna-boot/tests/ovmf/run.sh
```

The script builds the direct `luna-init` artifact and a small test System Image,
then constructs the EFI/SYSTEM/DATA GPT disk and starts QEMU.

## Test disk layout

```text
EFI    64 MiB
SYSTEM 256 MiB
DATA   128 MiB
```

SYSTEM contains:

```text
images/luna-test.squashfs
images/luna-test.toml
images/luna-test.init
kernels/test/bzImage
```

The `.squashfs` file is the System Image itself. The `.init` file is a separate
ELF64 `luna-init` artifact loaded and validated independently by `luna-boot`.

## Expected result

The serial console should contain messages equivalent to:

```text
Luna: handoff v1 accepted: ...
Luna: staging memory-resident luna-init for direct PID 1
Luna: executing luna-init as PID 1
Luna: luna-init is running as PID 1
```

The VM should remain running because `luna-init` currently waits as PID 1 and
reaps children. A failure before that point is a kernel/boot ABI problem rather
than a normal userspace service failure.

## Current limitation

This milestone does not yet start `luna-system-runtime`, construct the final
logical filesystem view or launch a graphical session. Those are subsequent
userspace stages.

The application launcher still uses Rust `CommandExt::pre_exec` for some
namespace setup. That remains a development limitation and is not the final
production child-creation primitive.
