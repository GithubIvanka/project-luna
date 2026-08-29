# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is now moving from contracts/prototypes into real backend implementation.

- Phases **1.1–1.5** are accepted and consolidated.
- Phase **1.6** is accepted through **1.6-HZ** and consolidated.
- The repository/Cargo audit and crate map are established.
- Foundation, domain, manager/runtime and integration-contract prototypes exist.
- `luna-boot.efi` is developed separately and has reached real kernel loading plus a test init handoff to `sh`.
- The remaining work is concentrated in real Linux namespace/materialization, durable state, update/rollback, final Bundle Format v1, and production security/signature infrastructure.

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
  ├── system/state
  ├── users/<user>/{home,data,config}
  └── cache
```

The four physical areas are **EFI / SYSTEM / DATA / SWAP**. SYSTEM is immutable/versioned and OS-managed; DATA contains mutable state.

Applications use immutable bundles and isolated logical filesystem views. `.lbp` is the transport/archive representation of a Bundle; RFC-0002 is still a separate design/acceptance task. A System Image is directly a SquashFS image and is not an `.lbp` bundle.

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

`luna-system-runtime` is the single system-wide runtime/supervisor. `UserSession` is the combined user/session domain entity. There is no separate `lunad` architectural component and no separate Session Manager.

`luna-app-manager` manages installation, update, removal, verification, migrations and package import. It does **not** own normal application execution.

`luna-security` is the central policy authority. Linux namespaces, mounts, cgroups and related kernel/filesystem mechanisms are implementation primitives used to enforce the Luna model rather than replacements for it.

## Current crate map

The current userspace workspace contains these architecture-defined crates:

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

The bootloader is intentionally outside the userspace workspace. `luna-log` and `luna-boot-state` remain separate conceptual boundaries until their implementation/API ownership requires dedicated crates.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md) for responsibility boundaries and current implementation status.

## Repository rule

The repository is not allowed to become a second architecture document.

A crate represents an architectural responsibility only when that responsibility is defined by `docs/ARCHITECTURE.md`. Existing code is reusable source material but does not override the architecture.

Historical phase records preserve traceability; they are not competing Sources of Truth.

## Development direction

```text
Architecture
    ↓
crate/API contract
    ↓
prototype
    ↓
backend implementation
    ↓
integration
    ↓
production hardening
```

## Current next targets

1. Real Linux namespace/materialization backend.
2. Durable persistent-state backend.
3. Real update/checkpoint/rollback engine.
4. Final `.lbp` / Bundle Format v1 and RFC-0002 acceptance.
5. Production signature/trust chain.

## License

Apache License 2.0.
