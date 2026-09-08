# ADR-0010 — Versioned `luna-init` artifact

**Status:** Accepted  
**Date:** 2026-09-08

## Context

Luna normal boot does not use an initramfs. `luna-boot.efi` must therefore provide the kernel with an executable `luna-init` image that can become PID 1 directly after the kernel finishes bootstrapping.

The artifact must be available to `luna-boot` without requiring a SquashFS implementation in the UEFI loader, while remaining tied to the selected System Image rather than to a particular kernel version.

## Decision

Each System Image version owns a standalone statically linked `luna-init` ELF artifact next to its SquashFS payload and manifest:

```text
SYSTEM/images/
├── luna-X.Y.Z.squashfs
├── luna-X.Y.Z.toml
└── luna-X.Y.Z.init
```

`luna-boot.efi` selects the three artifacts as one image target:

```text
System Image + manifest + luna-init
```

The selected kernel remains an independent artifact under `SYSTEM/kernels/<kernel-id>/`.

`luna-boot.efi` loads the `.init` bytes into reserved physical memory and passes their address, size and BLAKE3-256 digest through the `LUNA_INIT_IMAGE` record of `LunaBootHandoffV1`.

The `.init` suffix denotes the artifact role, not its binary format. The payload is strictly an ELF64 executable for x86-64.

The Linux kernel validates the object again and directly executes it as the first userspace process. No initramfs archive is created or used for this purpose.

## Rationale

This keeps the three important identities separate:

```text
System Image identity  → userspace environment
Kernel identity        → Linux kernel
luna-init identity     → boot/userspace supervisor for that image
```

The init artifact is not embedded in the kernel, so a kernel update does not implicitly rebuild or replace the userspace supervisor.

The artifact is not extracted from SquashFS by `luna-boot`, so the UEFI loader does not need to become a SquashFS reader just to start the first userspace process.

The artifact is also not a global `SYSTEM/init/luna-init` object: a rollback must reproduce the complete boot-critical userspace tuple belonging to the selected System Image.

## Consequences

Positive:

- no initramfs dependency in the normal boot path;
- no requirement for `luna-boot` to parse SquashFS;
- deterministic pairing of image and initial userspace supervisor;
- direct PID 1 semantics;
- independent kernel updates remain possible;
- the artifact naming remains consistent with Luna's role-oriented `squashfs` / `toml` naming.

Costs:

- every System Image version carries one additional boot-critical ELF;
- update tooling must validate and atomically install the `.init` artifact together with its image/manifest;
- `luna-boot` must validate the artifact before `ExitBootServices`;
- the kernel must repeat digest and ELF validation.

## Rejected alternatives

### Store `luna-init` only inside SquashFS

Rejected for the first implementation because it would require `luna-boot` to locate and read a file inside the selected SquashFS before Linux is running.

### One global `luna-init` for all images

Rejected because image rollback could pair an old System Environment with an incompatible supervisor.

### Put `luna-init` inside every kernel artifact

Rejected because it couples two independently versioned release domains.

### Use initramfs to transport `luna-init`

Rejected by the target architecture. It would reintroduce the exact transitional userspace layer Luna is removing.
