# ADR-0003 — Logical Linux Root

**Status:** Accepted  
**Phase:** 1.1  
**Date:** 2026-08-18

## Decision

Luna provides a conventional Linux-compatible logical `/` without physically recreating the Linux directory hierarchy in DATA or SYSTEM.

The runtime root is **RAM-backed**. The visible Linux `/` is created as a virtual/runtime filesystem root (currently `tmpfs`) inside the application's private namespace. The directory used as the mountpoint on persistent storage is only a staging/mountpoint object; it is not the backing store for the root filesystem.

The immutable System Image remains a physical SquashFS payload on the hidden SYSTEM partition. It is **not mounted as the application's `/`** and is **not copied wholesale into RAM**. Required system resources are exposed to the logical root through explicit profile/mapping operations and may be supplied lazily from the System Image.

The SYSTEM partition is a system-internal storage area. Ordinary users must not receive a filesystem view that exposes SYSTEM, its System Images, or its kernel inventory. It is not part of the user's normal file-manager namespace and is not mounted as user-visible storage.

## Per-application Linux environment

Every application runs against a **Linux-shaped virtual environment**. The process is presented with the conventional filesystem namespaces expected by Linux software, but visibility and access are independently controlled by Luna's authorization and namespace layers.

A path being present in the logical `/` does not grant unrestricted access to the corresponding physical resource. The application receives only:

- its own declared and authorized resources;
- explicitly selected trusted system resources from `RuntimeProfile`;
- explicitly authorized runtime/pseudo-filesystems;
- explicitly granted external capabilities such as network, clipboard, devices, or host integration.

This establishes the rule:

```text
visible path != granted resource
capability request != capability grant
```

An application may therefore see conventional paths such as `/etc`, `/usr`, `/lib`, `/tmp`, `/proc`, `/sys` and `/dev` while each path is backed only by resources selected for that launch. Access to host files, other users, other applications, devices, or external services is denied unless a separate policy grant makes it available.

Capabilities are not implicit extensions of the filesystem. For example, a `network` grant provides the network capability selected by the runtime; it does not expose the host filesystem or arbitrary host namespaces.

The logical root is **per application execution**, not a global root shared by all applications.

## Runtime model

```text
physical storage / host resources

SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/kernels/*
DATA/*
host services / devices / network
        │
        │ authorization + explicit trusted mappings/capabilities
        ▼
private application namespace
        │
        └── RAM-backed logical `/`
                │
                ├── conventional Linux directories
                ├── selected system resources
                ├── application-owned resources
                ├── runtime pseudo-filesystems
                └── explicitly granted capabilities
```

The physical SYSTEM payload remains the source of immutable system content; the logical `/` is a separate runtime composition. No application receives the SYSTEM filesystem itself as a normal filesystem tree.

## `luna-root`

`luna-root` owns logical-root construction and controlled path mapping. It does not own application lifecycle, sessions, updater logic or recovery logic.

## Compatibility paths

Paths such as `/etc`, `/home`, `/usr`, `/lib`, `/bin` and `/var` are logical compatibility interfaces. Their physical backing is selected by policy rather than by mirroring DATA or SYSTEM into `/`.
