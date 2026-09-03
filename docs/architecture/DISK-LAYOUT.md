# Project Luna — Disk and Storage Architecture

**Status:** accepted architecture; implementation details partially open
**Authority:** `docs/ARCHITECTURE.md`, accepted decision ledger and storage ADRs

## 1. Physical partition model

A Luna installation is divided into four logical areas:

```text
Disk
├── EFI
├── SYSTEM
├── DATA
└── SWAP
```

`SWAP` is optional and may be a partition, file or ZRAM. EFI and SYSTEM are OS-managed. DATA is the normal mutable/user-visible storage area.

The accepted design also permits EFI/SYSTEM and DATA/SWAP to reside on different physical disks.

## 2. EFI

Purpose: UEFI boot infrastructure only.

Canonical content:

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

EFI is not an ordinary user storage area. Users should not need to browse or edit it during normal operation.

## 3. SYSTEM

Purpose: immutable/versioned OS payload and kernels.

Canonical logical structure:

```text
SYSTEM/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
└── kernels/
    ├── <kernel-version>/
    │   ├── bzImage
    │   ├── System.map
    │   ├── config
    │   └── release
    └── current -> <selected-version>
```

The exact kernel metadata directory schema is an implementation contract and must remain compatible with `luna-boot` and `luna-kernel-manager`.

SYSTEM is OS-managed and is not exposed as a normal user filesystem. Administrative tooling may provide controlled access later; commands/permissions must not be invented without a corresponding contract.

## 4. System Images

A System Image is **directly a SquashFS filesystem image**. It is not an `.lbp` bundle and not a container holding SquashFS.

Canonical pair:

```text
images/
├── luna-1.0.0.squashfs
└── luna-1.0.0.toml
```

The manifest describes version and boot/kernel compatibility metadata. The detailed System Image contract is in `SYSTEM-IMAGE.md`.

## 5. DATA

The canonical current DATA layout is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

### `DATA/system/apps/`

Shared installed immutable application Bundles. Bundles are shared between users rather than copied per user.

### `DATA/system/drivers/`

Independently managed driver entities. Recovery must be able to disable/remove a broken driver without replacing the whole System Image. The exact taxonomy between kernel modules and mutable/user-installable drivers remains an open detail.

### `DATA/system/libs/`

Addressable shared libraries/dependencies. The architecture favors isolated versions rather than one conflict-prone global library directory.

### `DATA/system/volumes/`

OS-managed representation for attached external volumes. This is internal DATA state; the user-facing view is provided by the volume/file-manager layer.

### `DATA/system/config/`

Machine-wide mutable configuration. Exact per-file TOML layout is owned by the relevant configuration contract.

### `DATA/system/state/`

Persistent system state owned through `luna-state`. This is distinct from checkpoints and disposable cache.

### `DATA/users/<user>/home/`

The user's logical home storage.

### `DATA/users/<user>/data/`

Mutable application/user data. Application data is kept outside immutable Bundles.

### `DATA/users/<user>/config/`

User/application configuration overrides.

### `DATA/cache/`

Disposable cache. Cache cleanup must not be treated as deletion of durable user data or system state.

## 6. Logical `/` versus physical storage

Applications do not see this physical tree as their Linux root. Luna constructs a conventional logical `/` through the root-mapping and namespace layers.

```text
physical storage
      ↓
controlled mapping
      ↓
per-application namespace
      ↓
logical Linux /
```

The DATA hierarchy must therefore not be copied into `/` merely for Linux compatibility.

## 7. Filesystem choice

The current implementation direction uses **ext4 for the writable SYSTEM/DATA filesystem images** in the PC image builder, while System Images themselves use SquashFS. This separates writable metadata/state from immutable compressed system payloads.

The architectural contract must not infer additional filesystem guarantees beyond what is explicitly specified. In particular, Btrfs is accepted for checkpoint/rollback storage where applicable, but that does not make Btrfs the mandatory filesystem for the normal DATA layout.

## 8. Invariants

- SYSTEM images and kernels are not stored under DATA.
- DATA does not recreate a traditional Linux root hierarchy.
- System Images are immutable during normal operation.
- User/application mutable state remains independent of System Image version.
- EFI and SYSTEM are OS-managed.
- Physical paths are implementation details and must not leak into Bundle mapping declarations.
