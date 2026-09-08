# ADR-0009 — Luna Boot Handoff ABI v1 and Direct Initial Userspace

**Status:** Accepted  
**Date:** 2026-09-08

## Decision

Project Luna adopts a direct boot architecture with no separate initramfs userspace layer:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

`luna-boot.efi` prepares the platform and Luna-specific boot context before `ExitBootServices`. The Linux kernel is built with the boot-critical driver/filesystem/crypto support required to reach the boot storage and directly execute `luna-init`.

The kernel does not depend on an initramfs to discover the boot-critical drivers needed for SYSTEM or to start `luna-init`.

## Luna Boot Handoff ABI v1

The Luna-specific boot context is encoded as a typed `LunaBootHandoffV1` object and attached to the Linux x86 `setup_data` list.

The handoff is memory-resident and allocated before `ExitBootServices`. It is never stored as a file on EFI, SYSTEM or DATA.

The ABI uses:

```text
fixed header
    +
extensible typed records
```

Required v1 records:

```text
SYSTEM_PARTITION
DATA_PARTITION
SYSTEM_IMAGE
KERNEL_IDENTITY
BOOT_MODE
BOOT_STATE
```

The complete binary contract is defined in:

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
```

## Storage identity

SYSTEM and DATA are identified by GPT disk GUID + partition GUID. Linux device names such as `/dev/sda`, `/dev/sdb` and `/dev/nvme...` are not part of the Luna boot ABI.

`luna-init` resolves the stable partition identity to the actual Linux block device after kernel startup.

## Image integrity

The handoff records the selected System Image identity, manifest identity and expected image digest. This allows `luna-init` to validate that the physically resolved image is the image selected by `luna-boot`.

The handoff checksum validates handoff structure. Image authenticity/trust remains a separate security/update policy.

## Kernel model

The kernel remains an independent versioned artifact from the System Image.

The selected kernel is already executing when `luna-init` receives the handoff, so the handoff contains kernel identity/provenance rather than the kernel binary itself.

Boot-critical kernel dependencies are built in (`CONFIG_*=y`). Optional post-boot drivers may remain loadable modules.

Kernel-specific modules belong to the matching versioned kernel artifact:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

The module set is not a global shared pool.

## Hardware responsibility

`luna-boot` and the Linux kernel have separate responsibilities.

```text
luna-boot
    → UEFI-side discovery and Luna boot selection

Linux kernel
    → actual hardware initialization and kernel-owned platform information

luna-init
    → physical Luna resource model and System Environment construction
```

The handoff does not duplicate ACPI/E820 or other generic kernel hardware structures that already have standard Linux boot-protocol representations.

## Command-line policy

The Linux command line remains available for standard Linux kernel parameters and diagnostics. It is not the production transport for Luna storage/image identity.

The legacy `luna.system_device`, `luna.data_device` and `luna.system_image` parsing path is superseded by Handoff v1.

## Consequences

- no separate initramfs userspace stage;
- no classic `switch_root` boot architecture;
- no second PID-1 transition;
- `luna-init` is the normal userspace PID 1;
- `luna-system-runtime` is a child of `luna-init`;
- boot-critical drivers/filesystems are built into the Luna kernel;
- optional drivers can be shipped as kernel-matched modules;
- Luna boot identity is independent of `/dev/sdX` and `/dev/nvme*` naming;
- Luna-specific handoff is extensible without freezing one giant native struct.

## Implementation consequence

The current `develop` boot implementation contains transitional initramfs-era code. That code must now be replaced with the accepted direct boot model rather than extended.

In particular, the following are implementation targets for removal:

- external initramfs bundle containing `luna-init`;
- `pivot_root` used solely to leave an initramfs root;
- BusyBox as a mandatory early bootstrap dependency;
- command-line-only Luna storage/image selection;
- second `/sbin/init` transition.

The target is a single userspace bootstrap process:

```text
Linux kernel → luna-init PID 1
```
