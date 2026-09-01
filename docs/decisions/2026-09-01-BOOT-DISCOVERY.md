# Project Luna — Boot Discovery and External Boot Decision

**Дата:** 2026-09-01  
**Статус:** accepted implementation decision  
**Source of Truth:** `docs/ARCHITECTURE.md`

## 1. Ownership

`luna-boot.efi` owns UEFI boot-flow decisions. It does not own Linux userspace policy or application runtime semantics.

## 2. Normal System Image discovery

The bootloader does not keep a hardcoded list of ordinary Luna releases.

It discovers boot candidates from the SYSTEM partition:

```text
SYSTEM/images/*.squashfs
        +
SYSTEM/images/*.toml
        ↓
manifest validation
        ↓
SYSTEM/kernels/<version>/bzImage
        ↓
compatible kernel filtering
        ↓
System Image version ordering
        ↓
BootTarget catalog
```

The manifest belongs to its adjacent System Image. The `.squashfs` file remains the direct System Image filesystem format.

## 3. Compatible kernel rule

A normal System Image can be selected only when at least one SYSTEM kernel is declared compatible by its manifest.

The bootloader chooses the highest-version compatible kernel for that image in the current development implementation.

System Image and kernel remain independently versioned and updateable.

## 4. Default target and fallback

The highest-version compatible normal System Image is the default target.

If loading its kernel fails, the bootloader may try the next older compatible normal System Image without rewriting persistent boot state during that boot attempt.

Persistent boot-success/current-state semantics remain a separate durable-state task.

## 5. Boot Menu order

The fixed user-visible order is:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

`System Image selection` opens a second menu containing discovered normal System Images.

## 6. Recovery and Factory

Recovery and Factory are boot modes, not additional generic runtime components.

During the current development phase they may be represented by dedicated System Images identified by image metadata role. They are kept outside the ordinary System Image selection list.

If the corresponding image is absent or cannot be loaded, Luna reports the mode as unavailable rather than silently falling back to a shell or TTY.

## 7. External/USB boot

External Boot is a UEFI-only operation. It does not depend on the internal SYSTEM image being healthy.

The current implementation enumerates UEFI `SimpleFileSystem` devices other than the device hosting the running `luna-boot.efi`, constructs the standard path:

```text
EFI/BOOT/BOOTX64.EFI
```

and asks UEFI to load and start the external image.

Linux userspace and Luna runtime contracts are not involved in this operation.

## 8. GUI/verbose interaction

Normal boot shows the Luna GUI splash before kernel handoff.

Verbose Boot is available only from Boot Menu, is the second menu item, suppresses the splash and adds verbose kernel diagnostics for that boot.

## 9. No generic runtime component

Boot discovery does not introduce or depend on a `luna-runtime` architecture component. Runtime selection for applications is represented by the existing typed `RuntimeKind`/`RuntimeSpec` values and remains under the existing application runtime hierarchy.

## 10. Current implementation boundary

The discovery/parser code intentionally implements only the metadata needed by `luna-boot.efi` during the current bring-up. The final System Image manifest grammar, persistent current/failed boot state, recovery UX, and signed release verification remain separate hardening/specification work.
