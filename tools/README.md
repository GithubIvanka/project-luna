# Project Luna development tools

Scripts are run from the repository root. Each tool should have a clear scope and must not silently alter unrelated components.

## Build progress

`build-progress` keeps long build output in logs while showing concise progress in the terminal. Full logs are stored under `dist/logs/` or the relevant kernel log directory.

## Component builds

```bash
tools/build-component.sh luna-system-runtime --release
tools/build-luna-boot.sh
```

The bootloader result is `boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi`.

## PC image

```bash
tools/build-full-pc-image.sh
```

The current builder assembles separate EFI, `LUNA-SYS` and `LUNA-DATA` partitions into the development disk image. SWAP is separate.

`LUNA-SYS` contains versioned System Images, `luna-init` cores, kernels, configuration and recovery resources. A System Image is directly SquashFS; it is not an `.lbp` package.

## QEMU/OVMF

Use the current Luna kernel and actual boot artifacts. A script or successful build is not evidence of a complete boot until the image is executed and the result is recorded.

## Install

The PC-image installer is destructive and requires explicit confirmation. Always verify the target disk before invoking it.

## Documentation

The architecture authority is `docs/ARCHITECTURE.md`. Tool behavior must not introduce new architecture. Component-level instructions belong with the corresponding component documentation.
