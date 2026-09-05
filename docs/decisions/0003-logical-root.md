# ADR-0003 — Logical Linux Root

**Status:** Accepted  
**Phase:** 1.1  
**Date:** 2026-08-18

## Decision

Luna provides a conventional Linux-compatible logical `/` without physically recreating the Linux directory hierarchy in DATA or SYSTEM.

The runtime root is **RAM-backed**. The visible Linux `/` is created as a virtual/runtime filesystem root (currently `tmpfs`) inside the application's private namespace. The directory used as the mountpoint on persistent storage is only a staging/mountpoint object; it is not the backing store for the root filesystem.

The immutable System Image remains a physical SquashFS payload on the hidden SYSTEM partition. It is **not mounted as the application's `/`** and is **not copied wholesale into RAM**. Required system resources are exposed to the logical root through explicit profile/mapping operations and may be supplied lazily from the System Image.

The SYSTEM partition is a system-internal storage area. Ordinary users must not receive a filesystem view that exposes SYSTEM, its System Images, or its kernel inventory. It is not part of the user's normal file-manager namespace and is not mounted as user-visible storage.

## Runtime model

```text
physical storage

SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/kernels/*
        │
        │ explicit trusted mappings / lazy access
        ▼
private namespace
        │
        └── RAM-backed logical `/` (tmpfs/runtime filesystem)
                │
                ├── selected system resources
                ├── authorized application resources
                └── runtime pseudo-filesystems
```

The physical SYSTEM payload remains the source of immutable system content; the logical `/` is a separate runtime composition.

## `luna-root`

`luna-root` owns logical-root construction and controlled path mapping. It does not own application lifecycle, sessions, updater logic or recovery logic.

## Compatibility paths

Paths such as `/etc`, `/home`, `/usr`, `/lib`, `/bin` and `/var` are logical compatibility interfaces. Their physical backing is selected by policy rather than by mirroring DATA or SYSTEM into `/`.
