# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna is in the **architecture-to-implementation transition**.

- Phases **1.1–1.5** are accepted and consolidated.
- Phase **1.6** is accepted through **1.6-HZ** and consolidated into `docs/ARCHITECTURE.md`.
- The repository/Cargo audit is complete for the current baseline.
- The architecture-driven crate map is scaffolded in the workspace.
- The current work is explicit crate/API contract design.
- Scaffolding does **not** claim that the subsystems are implemented.

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Core architectural model

```text
UEFI
  ↓
luna-boot.efi
  ↓
SYSTEM
  ├── versioned System Images (SquashFS)
  └── versioned kernels
  ↓
logical Linux-compatible root
  ↓
DATA
  ├── system/apps
  ├── system/drivers
  ├── system/libs
  ├── system/volumes
  ├── system/config
  ├── users/<user>/{home,data,config}
  └── cache
```

The four physical areas are **EFI / SYSTEM / DATA / SWAP**. SYSTEM is immutable/versioned and OS-managed; DATA contains mutable state.

Applications use immutable bundles and isolated logical filesystem views. `.lbp` is the transport/archive representation; its final Bundle Format v1 details remain a separate RFC-0002 task. A System Image is directly a SquashFS image and is not an `.lbp` bundle.

## Runtime model

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

`luna-app-manager` manages installation, update, removal, verification, migrations and package import. It does **not** own normal application execution.

`luna-security` is the central policy authority. Linux namespaces, mounts and filesystem/resource-control mechanisms are implementation primitives, not substitutes for Luna's architectural model.

## Current crate map

The architecture-driven workspace currently contains these scaffolded boundaries:

```text
luna-common
luna-fs
luna-root-mapping
luna-config
luna-security
luna-state
luna-event
luna-bundle
luna-app-manager
luna-system-manager
luna-update-manager
luna-device-manager
luna-kernel-manager
luna-system-runtime
luna-user-session
luna-app-runtime
luna-cli
```

`luna-boot`, `luna-boot-state`, and `luna-log` remain separate architecture boundaries that are not yet workspace implementation targets. A future GUI client is also deferred.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md) for responsibility boundaries and deferred implementation details.

## Repository rule

The repository is not allowed to become a second architecture document.

A crate is introduced only when its responsibility and boundary are defined by the architecture. A scaffold establishes an ownership boundary; it is not a claim of completed functionality.

See [`STATUS.md`](STATUS.md), [`ROADMAP.md`](ROADMAP.md), and [`docs/phases/PHASE-1.6.md`](docs/phases/PHASE-1.6.md).

## Development direction

```text
Architecture
    ↓
crate/API contract
    ↓
RFC / specification where required
    ↓
prototype
    ↓
implementation
    ↓
integration
```

## License

Apache License 2.0.
