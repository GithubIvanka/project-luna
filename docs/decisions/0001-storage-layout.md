# ADR-0001 — DATA Storage Layout

**Status:** Accepted  
**Phase:** 1.0 / 1.1  
**Date:** 2026-08-18

## Decision

The user-visible DATA partition has exactly three top-level directories:

```text
DATA/
├── system/
├── users/
└── cache/
```

`DATA/system/` contains:

```text
system/
├── apps/
├── drivers/
├── libs/
└── volumes/
```

Each user has:

```text
users/<user>/
├── home/
├── data/
└── config/
```

The physical SYSTEM partition remains separate and contains System Images and kernels. EFI remains hidden and OS-managed.

## Rationale

The structure intentionally avoids recreating the traditional Linux directory zoo on disk while preserving Linux compatibility through the logical root/mapping layer.

`system/volumes/` belongs to the OS-managed portion because attached devices are managed by the system. `system/apps/` is shared between users so the same immutable application bundle is not duplicated per user.

## Supersedes

The Manifest 4 layout containing `DATA/data/` is superseded by this decision.
