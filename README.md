# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has entered architecture-driven backend implementation and hardening.

- Phases **1.1–1.6-HZ** are accepted and consolidated.
- Post-1.6 accepted clarifications are recorded in [`docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md`](docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md) pending safe consolidation into the large Source of Truth file.
- The repository/Cargo audit and crate map are established.
- Linux namespace/materialization primitives are implemented in `luna-namespace`.
- Durable system state is implemented in `luna-state` with `redb`.
- Checkpointed update/rollback orchestration exists in `luna-update-manager`.
- `luna-bundle` contains the LBP1 reader/writer implementation for the accepted RFC-0002 format.
- `luna-boot.efi` is developed separately and currently reaches the Linux kernel plus a test init handoff to `sh`.

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Core architectural model

```text
UEFI
  ↓
luna-boot.efi
  ↓
SYSTEM
  ├── versioned System Images (direct SquashFS)
  └── versioned kernels
  ↓
logical Linux-compatible root
  ↓
luna-system-runtime
  ├── UserSession A
  │   └── luna-app-runtime → ApplicationInstance(s)
  └── UserSession B
      └── luna-app-runtime → ApplicationInstance(s)
  ↓
DATA / mappings / security / Linux namespace primitives
```

The four physical areas are **EFI / SYSTEM / DATA / SWAP**.

`SYSTEM` contains immutable/versioned Luna System Images and kernels. A System Image is directly a `luna-X.Y.Z.squashfs` SquashFS filesystem image; it is never an `.lbp` Bundle.

`DATA` contains mutable system, user and cache state. The canonical model is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/<user>/
│   ├── home/
│   ├── data/
│   └── config/
└── cache/
```

## Bundles

Applications and installable components use immutable Luna Bundles.

```text
application.lbp
    ↓
Bundle Format v1
```

`.lbp` is the **transport/archive representation** of a Bundle. RFC-0002 — Bundle Format v1 is **Accepted**.

The accepted LBP1 format has a fixed 64-byte little-endian header, fixed 64-byte section entries, TOML manifest, deterministic TAR payload, BLAKE3-256 integrity/content identity, logical mapping declarations, request-only capabilities, and an optional Ed25519 signature section.

Different versions of the same application are independent immutable Bundles and may coexist.

## Runtime and isolation

`luna-system-runtime` is the single system-wide runtime/supervisor. `UserSession` is the combined user/session entity. There is no separate `lunad` architecture component and no separate normal Session Manager.

`luna-app-runtime` manages `ApplicationInstance` lifecycle and prepares an isolated execution environment. Every ApplicationInstance receives its own filesystem/mount namespace. The application sees a normal Linux-compatible logical `/`; physical Luna DATA/SYSTEM paths and mapping tables remain implementation details.

`luna-security` is the central policy authority. `luna-root-mapping` defines logical mapping semantics. `luna-namespace` contains Linux-specific namespace/materialization primitives. Linux namespaces, cgroups and related kernel mechanisms are enforcement primitives rather than replacements for the Luna architecture.

## Management boundaries

```text
luna-app-manager
    → install / import / verify / update / remove / migrate Bundles

luna-system-manager
    → system-state model and queries

luna-kernel-manager
    → kernel inventory / compatibility / queries

luna-update-manager
    → mutation coordination / checkpoints / apply / verify / rollback
```

Domain managers retain ownership of their own state; `luna-update-manager` does not become the owner of application, kernel or System Image semantics.

## Current crate map

The current userspace workspace contains these architecture-defined crates:

```text
luna-common
luna-fs
luna-root-mapping
luna-namespace
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

The bootloader is intentionally outside the userspace workspace.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md) for current responsibility boundaries.

## Development rule

The repository must not become a second architecture document.

`docs/ARCHITECTURE.md` is the primary Source of Truth. Historical phase records preserve traceability. The post-1.6 amendment records accepted decisions that have not yet been physically folded into the large Source of Truth document.

Accepted architecture decisions must not be silently changed by implementation work. When code reveals a real architectural conflict, that conflict must be documented and resolved explicitly.

## Current next targets

1. Complete Security-authorized namespace/process integration.
2. Connect durable state to `luna-system-runtime` and domain managers.
3. Make update journaling durable-before-mutation and record exact progress for interruption reconciliation.
4. Finish LBP1 conformance/security hardening and signature verification boundary.
5. Formalize System Image/kernel manifests, compatibility and persistent boot-state confirmation.
6. Integrate devices/volumes, resource control and end-to-end Linux/QEMU validation.

## License

Apache License 2.0.
