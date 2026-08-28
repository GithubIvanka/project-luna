# Implementation stages

## Stage 0 — contract

Accepted decisions are recorded in `BOOT-CONTRACT.md`:

- system partition: ext4;
- kernel: Linux x86_64 bzImage;
- boot menu: non-blocking B check, no two-second delay;
- target: System Image manifest + compatible kernel;
- normal failure: Factory pair;
- Factory failure: Recovery;
- System Image mounting belongs to early userspace, not `luna-boot`.

## Stage 1 — UEFI foundation

The foundation must identify the boot device, locate the system partition, provide a read-only ext4 block-device reader, prepare all allocations before `ExitBootServices`, and perform a final memory-map/exit sequence without post-exit UEFI calls.

## Stage 2 — selection

Selection resolves the current manifest, validates the image/kernel architecture and compatibility, handles the immediate B menu request, and implements normal → Factory → Recovery fallback.

## Stage 3 — Linux handoff

The loader validates the bzImage setup header, allocates the kernel/boot_params/command-line/initrd regions, creates the Linux zero page and E820 data, exits boot services, establishes the required 64-bit execution environment, and jumps to the kernel entry point.

### Current implementation state

The repository now contains the contracts and parsing/data layers for ext4, Linux setup headers, boot_params, E820 conversion, and the post-ExitBootServices handoff boundary. The remaining work is the hardware-facing ext4 block reader, real UEFI allocation/memory-map integration, complete target manifest parser, initramfs loading, and the final x86_64 assembly transition. These are intentionally not represented as fake working implementations.
