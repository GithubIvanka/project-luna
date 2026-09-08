# Luna Init Image Contract

**Status:** Accepted  
**Date:** 2026-09-08  
**Scope:** standalone `luna-init` ELF loaded by `luna-boot.efi` and directly executed by the Luna kernel integration

## 1. Purpose

`luna-init` is the first userspace executable and becomes PID 1 for the normal Luna boot.

It is delivered as a standalone executable artifact. It is not an initramfs archive, not a filesystem image and not a second root filesystem.

The boot chain is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
direct execution of luna-init ELF
  ↓
luna-init (PID 1)
```

## 2. Canonical artifact location

For an image with version `X.Y.Z`:

```text
SYSTEM/images/
├── luna-X.Y.Z.squashfs
├── luna-X.Y.Z.toml
└── luna-X.Y.Z.init
```

The `.init` artifact is versioned with the System Image because `luna-init` owns the transition from the boot context into the corresponding System Environment.

The `.init` suffix denotes the artifact role. The payload format is still strictly required to be ELF64; the filename suffix is not the format declaration.

The kernel remains independently versioned under `SYSTEM/kernels/`.

`luna-boot` derives the init artifact from the selected System Image identity. The file must not be discovered from arbitrary filesystem paths.

## 3. Build requirements

The production artifact must be:

- ELF64;
- little-endian;
- x86-64;
- statically linked;
- directly executable without a dynamic linker;
- built without a dependency on an initramfs filesystem;
- built for the Luna userspace ABI selected by the supported kernel/userspace toolchain.

The recommended production build target is:

```text
x86_64-unknown-linux-musl
```

The first production implementation must reject an ELF that contains `PT_INTERP` or otherwise requires a userspace dynamic loader before the System Environment exists.

## 4. ELF validation

`luna-boot.efi` validates the artifact before copying it to reserved physical memory.

The Luna kernel validates it again before execution.

At minimum both sides validate:

1. ELF magic and class;
2. endianness;
3. machine = x86_64;
4. supported ELF type (`ET_EXEC` and/or the explicitly accepted PIE form);
5. program-header table bounds and overflow safety;
6. every `PT_LOAD` range is inside the supplied byte object;
7. entry point is inside an executable `PT_LOAD` range;
8. no `PT_INTERP`;
9. no segment range wraps an integer boundary;
10. load alignment is sane for x86-64;
11. writable and executable segment permissions satisfy the Luna W^X policy;
12. total memory expansion implied by the loadable segments stays below the configured safety limit.

The kernel must not trust the bootloader's earlier validation as an authenticity decision.

## 5. Integrity

The artifact digest stored in `LUNA_INIT_IMAGE` is the BLAKE3-256 digest of the exact ELF byte range supplied to the kernel.

The kernel recomputes and verifies this digest before mapping the executable.

The handoff checksum does not replace this content digest and does not establish artifact authenticity.

Authenticity/signature policy belongs to the image/update trust chain.

## 6. Physical-memory ownership

Before `ExitBootServices` the bootloader loads the exact ELF byte range into reserved physical memory.

The range is described by the `LUNA_INIT_IMAGE` handoff record:

```text
physical_address
size
digest
flags
```

The range must not overlap:

- the Linux kernel image;
- `boot_params`;
- the Linux command line;
- the Luna handoff object;
- page tables or another incompatible boot object.

After `ExitBootServices` the bootloader performs no further mutation of this object.

The kernel retains the bytes until the initial userspace ELF has been successfully loaded and no kernel state references the original range.

## 7. Kernel execution model

The Luna kernel integration does not create a second userspace filesystem to execute the artifact.

The preferred implementation is a kernel-internal memory-backed executable object used only to reuse Linux's existing ELF loading/binfmt machinery. This object:

- has no pathname in the userspace filesystem;
- is not mounted or exposed as a root filesystem;
- exists only during initial execution;
- is inaccessible to ordinary users and applications;
- is released once the initial executable has been loaded.

A duplicated userspace ELF parser is explicitly not desired when the upstream ELF loader can be reused safely.

## 8. Process semantics

The Luna kernel launches this object as the initial userspace task.

Required result:

```text
PID 1 = luna-init
```

There is no earlier initramfs PID and no later `execve("/sbin/init")` replacement.

`luna-init` keeps PID 1 for the normal system lifetime and is responsible for starting `luna-system-runtime` as a child.

## 9. Boot-context delivery to luna-init

The kernel must provide the validated `LunaBootHandoffV1` context to `luna-init` without requiring a filesystem path.

The initial implementation will use a kernel-created read-only boot-context object exposed only to PID 1. The object is not a physical-device path and is not part of the logical root filesystem.

The exact userspace ABI for consuming this object must be frozen before the direct-init implementation lands. Until then, kernel and `luna-init` changes must not invent ad-hoc command-line parsing for Luna boot identity.

## 10. Failure policy

Failure to validate, map or execute the selected `luna-init` artifact is fatal for the current boot attempt.

The kernel must not silently execute:

- `/init` from an initramfs;
- `/sbin/init` from an unrelated root;
- a different `luna-init` discovered by pathname;
- a stale artifact that does not match the handoff digest.

Boot fallback remains a `luna-boot` / Boot State decision.

## 11. Non-goals

This contract does not define:

- the System Image filesystem layout inside SquashFS;
- application runtime;
- user/session management;
- kernel module packaging;
- update transaction format;
- signature format;
- the full userspace boot-context ABI.
