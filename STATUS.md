# Project Luna — Status

Last updated: 2026-08-28

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has moved into architecture-driven API implementation.

### Phase status

| Phase | Status |
|---|---|
| 1.1 | Accepted and consolidated |
| 1.2 | Accepted and consolidated |
| 1.3 | Accepted and consolidated |
| 1.4 | Accepted and consolidated |
| 1.5 | Accepted and consolidated |
| 1.6 | Accepted through 1.6-HZ and consolidated |
| Repository/Cargo audit | Completed |
| Crate map | Established |
| Foundation/domain API pass | **Completed** |
| Manager/runtime API baseline | **Completed** |
| Bundle Format v1 | Pending RFC-0002 design/acceptance |
| Integration test pass | Next |

## Current workspace

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

These crates are a mix of implemented domain/API contracts and intentionally incomplete subsystem scaffolds. API presence must not be confused with a finished operating-system implementation.

## API work completed in the current pass

Foundation/domain contracts:

- `luna-common`: `BundleId`, `ComponentId`, `UserId`, `Version`.
- `luna-fs`: low-level filesystem contract and host-backed implementation for tests.
- `luna-root-mapping`: per-namespace exact-file logical-to-physical mappings.
- `luna-config`: system/user/application scopes and application precedence.
- `luna-security`: central policy authority and authorization request model.
- `luna-state`: durable state key/value boundary.
- `luna-event`: event type/publication/subscription contracts.
- `luna-bundle`: internal bundle domain model; `.lbp` wire/archive format remains deferred.
- `luna-user-session`: session identity and lifecycle model.

Manager/runtime API baseline:

- `luna-system-manager`: current/factory System Image state query model.
- `luna-kernel-manager`: current/previous kernel selection query model.
- `luna-update-manager`: update operation/plan/execution model.
- `luna-app-manager`: application lifecycle operation model.
- `luna-device-manager`: device/volume query model.
- `luna-system-runtime`: single system-wide supervision boundary.
- `luna-app-runtime`: ApplicationInstance lifecycle and integration boundary.
- `luna-cli`: remains a thin client; final command grammar is not frozen.

## Important boundaries

`luna-app-manager` does not own normal application execution.

`luna-system-manager` owns the system state model; `luna-update-manager` executes mutations.

`luna-kernel-manager` owns kernel modeling/compatibility queries; update execution remains with `luna-update-manager`.

`luna-security` remains the central policy authority. Low-level kernel/filesystem/runtime mechanisms enforce policy decisions.

`luna-system-runtime` is the single system-wide runtime supervising multiple `UserSession` instances. `luna-app-runtime` owns application execution and `ApplicationInstance` lifecycle.

## Async direction

Tokio remains the accepted Rust async-runtime direction where an async runtime is actually required. Lower-level crates should not acquire Tokio merely by association.

## Verification note

The repository was updated through the foundation and manager/runtime API passes. A local `cargo test --workspace` was not executable from the connected environment because the container could not resolve `github.com`; this status therefore makes no claim of a locally executed build/test run. Cargo documents workspace-wide build/check operations, and resolver 3 is the current resolver for Rust 2024 virtual workspaces. citeturn265243search0turn265243search1

## Next work

1. Add integration tests across the new crate boundaries.
2. Review the manager/runtime API baseline against `docs/ARCHITECTURE.md` and remove any accidental overcommitments.
3. Design and accept RFC-0002 / Bundle Format v1 separately.
4. Begin higher-risk prototypes: namespace/materialization, persistence, update transactions, and boot compatibility.
