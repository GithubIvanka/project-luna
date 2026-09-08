# ADR-0012 — `luna-init` boot-context channel

**Status:** Accepted
**Date:** 2026-09-08

## Context

`luna-boot.efi` supplies a structured `LunaBootHandoffV1` object to the Luna kernel through x86 `setup_data`. The first userspace process must receive the validated context without depending on a filesystem path, initramfs, ad-hoc command-line options or a new syscall ABI.

The mechanism must remain kernel-internal and must not create a new root filesystem or expose physical SYSTEM/DATA paths.

## Decision

The Luna kernel creates a read-only anonymous file object containing the validated serialized `LunaBootHandoffV1` and installs it in the initial `luna-init` file descriptor table as:

```text
FD 3 = Luna boot-context
```

The descriptor is positioned at offset zero before userspace execution begins. Its contents are the exact handoff bytes validated by the kernel.

`luna-init` consumes the descriptor during early initialization and closes it before creating or launching normal child processes.

## Rationale

A fixed read-only FD has several advantages:

- no user-visible pathname is required;
- no dependency on the logical root filesystem;
- no custom syscall is required;
- no collision with Linux ELF auxiliary-vector namespace is introduced;
- the existing Linux file/read semantics can be reused;
- the channel naturally carries the variable-length typed handoff object.

The object is an internal boot-context transport, not a filesystem or persistent storage layer.

## Invariants

1. FD number `3` is reserved for the boot context during initial `luna-init` startup.
2. The descriptor is read-only.
3. The descriptor starts at offset zero.
4. The descriptor contains only the validated `LunaBootHandoffV1` byte sequence.
5. The object has no filesystem pathname.
6. The object is not inherited intentionally by later runtime components; `luna-init` closes it before normal child creation.
7. Missing or invalid FD 3 is a fatal direct-init boot error.

## Rejected alternatives

### Custom Luna syscall

Rejected because the initial boot context does not require a new syscall ABI and a fixed FD can use established kernel/userspace primitives.

### Custom ELF auxiliary-vector entries

Rejected for the first implementation to avoid introducing a second ABI namespace into ELF process startup and to keep the boot context available as a normal byte stream.

### Environment variables or argv

Rejected because Luna boot identity is structured binary data, not string configuration.

### Filesystem path

Rejected because the context must be available before a logical root exists and must not depend on physical storage paths.
