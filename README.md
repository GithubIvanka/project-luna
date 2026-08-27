# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna is in the **architecture-to-implementation transition**.

- Phases **1.1–1.5** are accepted and consolidated.
- Phase **1.6** is accepted through **1.6-HZ** and consolidated into `docs/ARCHITECTURE.md`.
- The repository/crate audit has started from the real `main` branch.
- The repository currently contains only the surviving `luna-common` implementation; obsolete empty crates were intentionally removed.
- No bootloader, runtime, bundle implementation, System Image implementation, or manager subsystem is claimed as implemented.

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

Applications use immutable bundles and isolated logical filesystem views. `.lbp` is the transport/archive Bundle Format; a System Image is directly a SquashFS image and is not an `.lbp` bundle.

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

## Repository rule

The repository is not allowed to become a second architecture document.

A crate is introduced only after:

1. its responsibility is defined;
2. its boundary is defined;
3. inputs/outputs and dependencies are known;
4. persistent state is identified;
5. the API/error model is defined;
6. the crate is ready for real development.

See [`STATUS.md`](STATUS.md), [`ROADMAP.md`](ROADMAP.md), and [`docs/phases/PHASE-1.6.md`](docs/phases/PHASE-1.6.md).

## Development direction

```text
Architecture
    ↓
RFC / specification
    ↓
crate/API contract
    ↓
prototype
    ↓
implementation
    ↓
integration
```

## License

Apache License 2.0.
