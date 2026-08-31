# Project Luna — Implementation Audit 2026-08-31

## Scope

This audit compares the current `main` repository, the accepted Phase 1.6 decisions, RFC-0002, and the architectural Source of Truth.

## Current repository

`main` contains the current 17 userspace/workspace crates plus the separate `boot/luna-boot` tree. `Cargo.toml` uses resolver 3 and lists the current crates, including `luna-root-mapping`, `luna-namespace`, `luna-state`, `luna-bundle`, the managers, runtimes and CLI.

PR #2 from Copilot is merged. It only changed `components/luna-bundle/src/lbp_v1.rs` for clippy/test cleanup without changing the intended LBP1 wire layout.

## Source of Truth

`docs/ARCHITECTURE.md` remains authoritative. The accepted post-1.6 clarifications are additionally preserved in `docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md` until the large Source of Truth file can be consolidated safely without losing historical material.

The current amendment explicitly records the later accepted boundaries for `luna-system-runtime`, `UserSession`, logical root composition, security/mapping, DATA layout, redb, update coordination, Linux namespaces, filtered `/dev`, IPC, Recovery vs Factory, and RFC-0002.

## Completed in this pass

### `luna-namespace`

Logical-root composition now uses OverlayFS with an immutable lower System Image and writable runtime upper/work state. This replaces the previous unsafe sequence that attempted to create missing mapping targets inside a read-only bind of the lower root.

The namespace backend also validates relative `/dev` exposure names and keeps policy ownership outside the crate.

### `luna-security`

The policy contract now explicitly contains the separate `Visibility` permission dimension and typed constrained decisions:

```text
Constraint::ReadOnly
Constraint::PathLimited(...)
Constraint::DeviceLimited(...)
```

A constrained decision is no longer represented by an untyped scope string.

### `luna-app-runtime`

A security-aware namespace preparation entry point now requires a `PolicyAuthority` and an explicit set of authorization requests before calling Linux namespace materialization. `Deny` and unresolved `Ask`/`Constrained` decisions fail closed at this boundary.

The older contract-only preparation method remains for compatibility with earlier prototypes, but production integration should use the security-aware boundary.

### `luna-state`

`RedbStateStore` remains the accepted durable backend under:

```text
DATA/system/state/luna-state.redb
```

Mutations and global revision commit atomically in one database transaction. No additional Luna-specific WAL is layered over redb.

### `luna-update-manager`

The journal is now written before the domain `prepare` call. Per-operation progress is durably recorded with:

```text
updates/<id>/phase
updates/<id>/plan
updates/<id>/applied
updates/<id>/inflight
```

The engine records an in-flight step before mutation and marks it applied only after success. Interruption reconciliation conservatively rolls back both recorded completed steps and a possibly-applied in-flight step. This gives the domain backend a clear idempotent-rollback contract.

## Remaining high-risk gaps

### Security-to-namespace mapping semantics

The explicit security-aware entry point exists, but mapping declarations are not yet automatically transformed into fine-grained authorization requests. This remains an integration task so that the exact mapping/resource identity is checked by `luna-security` before each writable/device exposure.

### Real process launch/supervision

`luna-app-runtime` still stops at namespace preparation. Real child-process creation, exec, lifecycle monitoring and restart/recovery remain owned by the higher runtime/supervisor layer and are not hidden inside `luna-namespace`.

### Update domain backends

`luna-update-manager` now has a stronger durable journal, but concrete backends are still needed from `luna-system-manager`, `luna-kernel-manager` and `luna-app-manager`. Domain ownership must not move into `luna-update-manager`.

### LBP1 conformance

RFC-0002 is Accepted. The codec remains the reference implementation, but the following still need completion before calling it production-ready:

- canonical manifest spelling must be identical between RFC and code;
- complete malformed-input matrix;
- transport-independent ContentIdentity tests;
- concrete Ed25519 signature record encoding and verification;
- trust-store integration outside `luna-bundle`.

### System Image / kernel specification

The direct SquashFS System Image model and `system/kernels/` naming are fixed. Detailed image manifest, kernel metadata, compatibility resolution, and persistent boot-success confirmation remain next.

### Device integration

`/dev` remains filtered. `luna-device-manager` still needs to provide the authorized device/volume objects that the namespace backend exposes.

### IPC / events

The architectural choice is Unix-domain sockets with a Luna typed binary protocol. Final framing, API negotiation, event persistence boundaries and production transport remain implementation work.

## Verification note

The available GitHub connector does not expose a fresh workflow run for the latest commits in this pass, so this audit intentionally does not claim that the newest code has passed CI. The repository's existing workflow remains the authoritative build/test gate.

## Next implementation order

1. Run and inspect fresh CI for the latest commits.
2. Complete fine-grained security-to-mapping authorization.
3. Add real process launch and supervision through `luna-system-runtime`.
4. Connect concrete update backends to domain managers.
5. Complete LBP1 canonical field/signature verification and malformed-input tests.
6. Connect durable state to runtime/system-manager ownership.
7. Formalize System Image/kernel metadata and boot-success state.
8. Integrate filtered `/dev`, IPC/events, resources and end-to-end QEMU/Linux tests.
