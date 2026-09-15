# luna-boot.efi

`luna-boot.efi` is the UEFI bootloader for Project Luna.

## Canonical path

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux kernel
 ↓
luna-init
 ↓
luna-system-runtime
```

## Storage discovery

The loader verifies that the EFI System Partition and `LUNA-SYS` are on the same physical disk, then reads the root of that `LUNA-SYS` partition:

```text
/images
/cores
/kernels
/config
/recovery
```

`LUNA-DATA` may be on the same disk or another disk. Normal DATA discovery uses the disk GUID and partition GUID stored in `/config/luna-data.toml`. If the bound DATA disk/partition is missing, normal boot enters Recovery. Recovery provides the search/selection utility; multiple valid candidates require explicit user choice, and the selected GUIDs may be written back to `luna-data.toml`.

## Target discovery

A normal candidate is the complete `System Image + luna-init + kernel` target. The System Image manifest selects compatible init cores; the init manifest selects compatible kernels. `luna-boot.efi` loads the kernel and `.init`, then the kernel starts `luna-init`, which materializes the System Image. The highest-version compatible image is the default normal target unless durable state or explicit menu selection says otherwise.

## Boot Menu

Normal boot has no artificial menu delay. Held `B` opens the exceptional Boot Menu with Continue, Verbose Boot, System Image selection, Recovery, Factory and External/USB boot.

## Handoff

The loader prepares Linux boot parameters plus `LunaBootHandoffV1`, loads the selected `.init` ELF into boot-reserved memory, writes `LunaBootAttempt` once immediately before `ExitBootServices`, then transfers control to Linux.

## Scope

The loader owns UEFI-time discovery/selection/handoff. User sessions, applications, Bundle installation and ordinary userspace service management belong to later components.

Detailed logic: `docs/architecture/BOOT-PATH.md`.
