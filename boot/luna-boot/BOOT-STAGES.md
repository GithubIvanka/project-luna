# luna-boot implementation status

## Stage 0 — contract

- UEFI entry receives the image handle and system table.
- ESP access is bound to `LoadedImage.device()`.
- `system` is ext4 and is read through UEFI Block I/O; it is not treated as an EFI filesystem.
- Boot key detection is one non-blocking input sample; there is no fixed delay.
- A target is a System Image manifest plus a compatible Linux x86_64 `bzImage`.
- `luna-boot` does not mount SquashFS.
- `luna-boot` does not load an initramfs userspace environment.
- Luna-specific boot context is carried by `LunaBootHandoffV1` through Linux `setup_data`.
- SYSTEM and DATA are identified by GPT disk/partition identities, not Linux device names.

## Stage 1 — UEFI foundation

Implemented:

- Correct image-device binding for ESP access.
- Immediate B/b check without resetting the firmware input queue.
- Linux loader code no longer performs UEFI operations after the handoff boundary.

Remaining:

- Complete physical Block I/O adapter and GPT/SYSTEM partition resolver.
- Allocate reserved handoff memory before `ExitBootServices`.

## Stage 2 — target selection

Implemented direction:

- No persistent boot timeout.
- Image/kernel target is selected as a compatible pair.
- Factory remains the immutable fallback pair.
- Target paths use plain Linux `bzImage`, not `.efi`.

Remaining:

- Read authoritative System Image manifest from ext4.
- Resolve `current` and kernel compatibility from manifest.
- Integrate complete recovery selection.
- Produce the final image digest and manifest identity for Handoff v1.

## Stage 3 — Luna Boot Handoff

Target implementation:

- Construct fixed ABI header plus typed records.
- Add `SYSTEM_PARTITION` and `DATA_PARTITION` records using GPT identities.
- Add `SYSTEM_IMAGE` identity/digest record.
- Add `KERNEL_IDENTITY` record.
- Add `BOOT_MODE` and `BOOT_STATE` records.
- Compute and validate Handoff checksum.
- Attach Handoff to Linux `setup_data`.
- Keep the handoff memory reserved through early kernel consumption.

Canonical contract:

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
```

## Stage 4 — Linux kernel handoff

The production kernel must:

- validate the Luna setup_data record;
- preserve the handoff until Luna early userspace has consumed it;
- expose a validated Luna boot context to `luna-init`;
- launch `luna-init` directly as initial userspace;
- require no external initramfs for the Luna boot path.

Boot-critical storage/filesystem/crypto drivers are built into the Luna kernel. Optional post-boot functionality may remain as loadable modules.

## Stage 5 — direct `luna-init`

Target:

```text
Linux kernel
    ↓
luna-init (PID 1)
    ↓
System Environment
    ↓
luna-system-runtime
```

`luna-init` must use the validated Handoff rather than legacy `luna.system_*` command-line parsing.

The transitional `pivot_root`, BusyBox bootstrap and second `/sbin/init` implementation must be removed rather than extended.

## Stage 6 — kernel modules

Kernel-specific optional modules are packaged with the matching kernel artifact:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

The module set is not a global pool and is not required for the boot-critical path.

## Stage 7 — final x86_64 handoff

Remaining hardware-facing work:

- allocate/load kernel payload according to the Linux boot protocol;
- construct complete `boot_params`;
- populate required firmware/memory fields;
- obtain final UEFI memory map immediately before `ExitBootServices`;
- construct and link `LunaBootHandoffV1`;
- perform the architecture-specific x86_64 entry transition;
- test under OVMF/QEMU without an initramfs.

The branch must not claim end-to-end no-initramfs bootability until these hardware-facing steps and the Luna kernel integration are actually implemented and tested.
