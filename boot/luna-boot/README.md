# luna-boot

`luna-boot` is Project Luna's standalone UEFI bootloader. It is intentionally
separate from the normal userspace Cargo workspace.

## Current design

```text
UEFI
  ↓
luna-boot.efi
  ↓
GPT → system partition → ext4
  ↓
System Image target + compatible Linux bzImage
  ↓
boot_params + E820 + initramfs
  ↓
ExitBootServices
  ↓
identity paging + x86_64 Linux entry
  ↓
Linux
```

The boot menu is not a timed window. `luna-boot` samples the UEFI input queue
once at startup: a queued `B` opens the menu; otherwise normal boot proceeds
without an artificial delay.

The System Image itself is never interpreted by the bootloader. It remains a
`*.squashfs` file; early Linux userspace is responsible for constructing the
logical root.

## UEFI API

The crate uses the current stable `uefi` crate rather than the original Qwen
prototype dependency set. The current repository target is `uefi 0.40`.

## Build

```bash
cd boot/luna-boot
cargo build --release --target x86_64-unknown-uefi
```

The configured Rust toolchain automatically requests the UEFI target.

## OVMF/QEMU test

The repository contains `tests/ovmf/run.sh`. The test constructs a temporary
GPT disk containing an EFI System Partition and an ext4 partition named
`system`, then launches QEMU with OVMF.

Provide:

```bash
export OVMF_CODE=/path/to/OVMF_CODE.fd
export OVMF_VARS=/path/to/OVMF_VARS.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export LUNA_TEST_INITRD=/path/to/initramfs.img
export LUNA_TEST_SQUASHFS=/path/to/luna-test.squashfs
```

Then:

```bash
cd boot/luna-boot
tests/ovmf/run.sh
```

Required host tools: `qemu-system-x86_64`, `sgdisk`, `mkfs.ext4`, `mkfs.fat`,
`mformat`, `mmd`, `mcopy`, `dd`.

## Architectural boundary

`luna-boot` loads Linux and supplies the boot protocol state. It does not mount
SquashFS, manage DATA, or implement the Luna logical root. Those responsibilities
belong to early userspace (`luna-init` and related components).
