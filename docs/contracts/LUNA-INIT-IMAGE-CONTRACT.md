# Luna Init Core Contract

**Status:** Accepted  
**Date:** 2026-09-12  
**Scope:** standalone `luna-init` core artifact loaded by `luna-boot.efi` and directly executed by the Luna kernel integration

## 1. Purpose

`luna-init` is the first Luna userspace executable and becomes PID 1 for the normal Luna boot.

It is delivered as a standalone versioned core artifact. It is not an initramfs archive, not a filesystem image and not a second root filesystem.

The boot chain is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
direct execution of luna-init core
  ↓
luna-init (PID 1)
```

## 2. Canonical artifact location

`luna-init` is independent from the selected System Image and kernel lifecycle.

For an init core version `X.Y.Z`:

```text
SYSTEM/cores/
└── luna-X.Y.Z.init
```

The selected System Image remains a separate artifact:

```text
SYSTEM/images/
├── luna-A.B.C.squashfs
└── luna-A.B.C.toml
```

The kernel remains independently versioned:

```text
SYSTEM/kernels/<kernel-id>/bzImage
```

A System Image does not contain, own or retain its `luna-init` core. The boot target is the compatible combination of:

```text
System Image + Kernel + luna-init core
```

The `.init` suffix identifies the Luna artifact role. It is intentionally not a user-facing statement of the executable's internal binary representation.

## 3. Build requirements

The production artifact must be:

- a standalone executable suitable for direct initial userspace execution;
- little-endian;
- x86-64 for the current PC target;
- statically linked;
- directly executable without a dynamic linker;
- built without a dependency on an initramfs filesystem;
- built for the Luna userspace ABI selected by the supported kernel/userspace toolchain.

The current production implementation uses an ELF64 executable internally because the Linux kernel's existing executable loading machinery is reused. The `.init` filename is the Luna artifact convention and does not expose or rename that internal representation.

The recommended production build target remains:

```text
x86_64-unknown-linux-musl
```

The first production implementation must reject an executable that contains `PT_INTERP` or otherwise requires a userspace dynamic loader before the System Environment exists.

## 4. Executable validation

`luna-boot.efi` validates the selected `.init` artifact before loading it into reserved physical memory.

The Luna kernel validates it again before execution.

For the current ELF64 execution representation, both sides validate at minimum:

1. ELF magic and class;
2. endianness;
3. machine = x86_64;
4. supported ELF type (`ET_EXEC` and/or an explicitly accepted PIE form);
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

The artifact digest stored in `LUNA_INIT_IMAGE` is the BLAKE3-256 digest of the exact `.init` byte range supplied to the kernel.

The kernel recomputes and verifies this digest before mapping the executable.

The handoff checksum does not replace this content digest and does not establish artifact authenticity.

Authenticity/signature policy belongs to the image/update trust chain.

## 6. Physical-memory ownership

Before `ExitBootServices` the bootloader loads the exact `.init` byte range into reserved physical memory.

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
- the E820 extension object;
- page tables or another incompatible boot object.

After `ExitBootServices` the bootloader performs no further mutation of this object.

The kernel retains the bytes until the initial userspace executable has been successfully loaded and no kernel state references the original range.

## 7. Kernel execution model

The Luna kernel integration does not create a second userspace filesystem to execute the artifact.

The preferred implementation is a kernel-internal memory-backed executable object used only to reuse Linux's existing executable loading/binfmt machinery. This object:

- has no pathname in the userspace filesystem;
- is not mounted or exposed as a root filesystem;
- exists only during initial execution;
- is inaccessible to ordinary users and applications;
- is released once the initial executable has been loaded.

A duplicated userspace ELF parser is not desired when the upstream executable loader can be reused safely.

## 8. Process semantics

The Luna kernel launches this object as the initial userspace task.

Required result:

```text
PID 1 = luna-init
```

There is no earlier initramfs PID and no later `execve("/sbin/init")` replacement.

`luna-init` keeps PID 1 for the normal system lifetime and is responsible for starting `luna-system-runtime` as a child.

## 9. Boot-context delivery to luna-init

The kernel must provide the validated `LunaBootHandoffV1` context to `luna-init` without requiring a filesystem path, command-line parsing or a custom syscall.

The canonical initial channel is a kernel-created read-only anonymous file object installed into the initial userspace file descriptor table as:

```text
FD 3 = Luna boot-context
```

FD 3 is fixed by contract for the initial `luna-init` process. It is opened read-only, starts at offset zero and contains the serialized `LunaBootHandoffV1` bytes exactly as validated by the kernel.

The object:

- has no pathname;
- is not backed by SYSTEM or DATA;
- is not part of the logical root filesystem;
- is accessible only through the initial process's inherited FD table;
- must be consumed and closed by `luna-init` before it creates or launches normal child processes.

The kernel must install FD 3 before transferring control to the executable entry point. The handoff object must be valid for the lifetime of the initial `luna-init` process until it closes the descriptor.

No Luna-specific boot identity may be reconstructed from `argv`, environment variables or legacy `luna.*` kernel command-line options.

## 10. Failure policy

Failure to validate, map or execute the selected `luna-init` core artifact is fatal for the current boot attempt.

The kernel must not silently execute:

- an initramfs `/init`;
- `/sbin/init` from an unrelated root;
- a different `luna-init` discovered by pathname;
- a stale core artifact that does not match the handoff digest.

Boot fallback remains a `luna-boot` / Boot State decision.

## 11. Retention and lifecycle independence

`luna-init` cores have their own retention policy.

Removing a System Image must not require removing or preserving a `luna-init` core merely because that image previously used it. Likewise, removing an obsolete `luna-init` core must not remove a System Image.

A core may be retained because it is required by one or more compatible boot targets, or because the configured core retention policy preserves it for fallback.

The bootloader must never delete the core selected for the currently active boot attempt.

## 12. Non-goals

This contract does not define:

- the System Image filesystem layout inside SquashFS;
- application runtime;
- user/session management;
- kernel module packaging;
- update transaction format;
- signature format;
- the full userspace boot-context ABI.
