# Implementation stages

## Stage 0 — contract

Accepted decisions:

- `system` is ext4;
- kernel is a Linux x86_64 `bzImage`;
- boot menu is entered only when B is already present in the UEFI input queue; there is no two-second delay;
- a resolved target is a System Image plus a compatible kernel and optional initramfs;
- normal boot failure falls back only to the immutable Factory pair;
- System Image mounting belongs to early userspace, not `luna-boot`.

The exact physical representation of `current` and the final manifest schema remain separate architecture work and are therefore not silently invented here.

## Stage 1 — UEFI foundation

Implemented:

- modern `uefi` 0.40 API;
- boot-device discovery from the loaded image device path;
- parent-disk Block I/O discovery;
- GPT discovery by canonical `system` partition name;
- read-only ext4 superblock, inode, directory and extent reader;
- all loader allocations use UEFI `LOADER_DATA` pages;
- final memory map and `ExitBootServices` use `uefi::boot::exit_boot_services`.

## Stage 2 — selection

Implemented in prototype form:

- immediate B sampling with no wait;
- target model contains System Image + kernel + initramfs;
- automatic fallback is Factory only.

Still deliberately provisional:

- manifest discovery and `current` physical format;
- full Boot Menu recovery/USB actions;
- final Recovery environment handoff.

## Stage 3 — Linux handoff

Implemented:

- Linux x86 boot-protocol header parsing with current documented offsets;
- relocatable 64-bit `bzImage` validation;
- kernel placement below 4 GiB;
- zero page and command line allocation;
- initramfs allocation;
- E820 population from the final UEFI map;
- loader-owned identity page tables covering the first 64 GiB;
- final x86_64 assembly transition with flat Linux boot GDT and `RSI = boot_params`.

## Test status

The repository contains the source-side OVMF/QEMU harness. A real QEMU run still has to be performed on a machine with QEMU + OVMF and a test `bzImage`, initramfs and SquashFS. This environment cannot execute QEMU against the user's local firmware/files.
