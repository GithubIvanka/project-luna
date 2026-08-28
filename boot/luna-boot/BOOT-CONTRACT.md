# luna-boot boot contract

This document is the implementation contract for the Project Luna UEFI bootloader.

## Firmware boundary

`luna-boot.efi` starts in UEFI Boot Services. It receives the image handle and system table from firmware. The bootloader must identify its own device and must not select an arbitrary `SimpleFileSystem` handle.

The production Luna system partition is ext4. `luna-boot` therefore uses UEFI Block I/O for the system device and a read-only ext4 reader. UEFI's Simple File System protocol is used only where the firmware exposes the ESP filesystem containing the bootloader itself.

## Boot key

There is no two-second boot delay. On entry, `luna-boot` performs a non-blocking read of the UEFI console input buffer. If `B`/`b` is already pending, Boot Menu is entered. Otherwise normal boot proceeds immediately.

## Target resolution

The selected target is a pair:

`System Image manifest + compatible Linux bzImage`

The manifest is authoritative for image version and kernel compatibility. `current` is the normal target. Factory is an immutable fallback pair.

## Kernel format

Linux is built as the normal x86_64 `arch/x86/boot/bzImage`. `luna-boot` implements the Linux x86 64-bit boot protocol rather than invoking the EFI stub.

The loader prepares `boot_params`, command line, initrd, and memory information, exits UEFI Boot Services, and transfers control to the 64-bit kernel entry point.

## System Image handoff

`luna-boot` does not mount SquashFS. The kernel receives the system-image selection through the kernel command line and an initramfs containing the Luna early userspace (`luna-init`). `luna-init` is responsible for finding the ext4 system partition, opening the selected `.squashfs`, constructing the logical root, and attaching DATA.

## Failure policy

Normal target failure causes selection of the immutable Factory System Image + Factory Kernel pair. Failure of both normal and Factory paths enters Recovery. Recovery must remain usable without DATA.

## Post-ExitBootServices rule

After `ExitBootServices`, no UEFI Boot Services protocol, allocator operation, console operation, logger requiring Boot Services, or UEFI filesystem access may occur. All boot data needed by Linux must already reside in memory owned by the loader/kernel handoff.
