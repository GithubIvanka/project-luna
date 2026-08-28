# luna-boot implementation status

## Stage 0 — contract

- UEFI entry receives the image handle and system table.
- ESP access is bound to `LoadedImage.device()`.
- `system` is ext4 and is read through UEFI Block I/O; it is not treated as an EFI filesystem.
- Boot key detection is one non-blocking input sample; there is no fixed delay.
- A target is a System Image manifest plus a compatible Linux x86_64 `bzImage`.
- Normal failure may fall back only to the immutable Factory pair.
- `luna-boot` does not mount SquashFS; `luna-init` owns System Image construction.

## Stage 1 — UEFI foundation

Implemented:

- Correct image-device binding for ESP access.
- Immediate B/b check without resetting the firmware input queue.
- Linux loader code no longer performs UEFI operations after the handoff boundary.

Remaining:

- Replace the legacy memory-map helper with the exact `uefi = 0.28` memory-map API.
- Add the physical Block I/O adapter and GPT/system-partition resolver.

## Stage 2 — target selection

Implemented:

- No boot timeout field.
- Factory is the only automatic fallback.
- Target paths use plain Linux `bzImage`, not `.efi`.

Remaining:

- Read the authoritative System Image manifest from ext4.
- Resolve `current` and kernel compatibility from the manifest.
- Integrate recovery selection.

## Stage 3 — Linux loader

Implemented:

- Linux x86 boot-protocol header validation.
- 64-bit protocol minimum validation.
- bzImage payload bounds validation.
- Initial `boot_params` representation.
- E820 data model.
- Explicit data-only handoff boundary.

Remaining:

- Allocate and place the protected-mode kernel payload.
- Allocate/initrd and command line below protocol limits.
- Populate the complete `boot_params` structure.
- Build final E820/EFI memory information.
- Obtain the final memory map immediately before `ExitBootServices`.
- Implement the architecture-specific x86_64 entry transition.

The branch intentionally does not claim to be bootable until these final
hardware-facing operations are implemented and tested under OVMF/QEMU.
