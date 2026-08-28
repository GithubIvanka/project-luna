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
| Foundation/domain API pass | **Completed for current first wave** |
| Manager/runtime API pass | Pending |
| Bundle Format v1 | Pending RFC-0002 design/acceptance |

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

The foundational/domain crates now have explicit contracts. Higher-level managers and runtimes remain intentionally smaller until their lower-level dependencies stabilize.

## Completed API work

- `luna-common`: shared `BundleId`, `ComponentId`, `UserId`, and `Version` value types.
- `luna-fs`: low-level filesystem operation boundary with host-backed implementation for tests.
- `luna-root-mapping`: per-namespace exact-file logical-to-physical mapping model.
- `luna-config`: system/user/application configuration scopes and application lookup precedence.
- `luna-security`: central authorization policy boundary.
- `luna-state`: durable state key/value boundary.
- `luna-event`: event publication/subscription contracts without selecting a broker.
- `luna-bundle`: internal bundle domain model without accepting `.lbp` wire/archive details.
- `luna-user-session`: explicit user-session identity and lifecycle state model.

## Important architectural boundaries

`luna-app-manager` manages application installation, update, removal, verification, migration and package import. It does not own normal application execution.

`luna-system-manager` owns system-state modeling/query responsibilities; `luna-update-manager` executes mutations.

`luna-kernel-manager` owns kernel modeling/compatibility queries; update execution remains with `luna-update-manager`.

`luna-security` remains the central policy authority. Filesystem/kernel primitives enforce the resulting restrictions.

`luna-system-runtime` is the single system-wide runtime supervising multiple `UserSession` instances. `luna-app-runtime` owns application execution and `ApplicationInstance` lifecycle.

## Async direction

Phase 1.6 accepted Tokio as the Rust async-runtime direction where an async runtime is actually required. Lower-level crates must not acquire Tokio merely by association.

## Verification note

The repository was audited and updated through the current API/domain pass. A local `cargo test --workspace` was not executable from the connected environment because the container could not resolve `github.com`; therefore this status does not claim a locally executed build/test run. Cargo documents `cargo check --workspace` and `cargo build --workspace` as workspace-wide operations, and resolver 3 is the current resolver for Rust 2024 virtual workspaces. citeturn265243search0turn265243search1

## Next work

1. Derive concrete manager/runtime APIs from the foundation contracts.
2. Add integration tests across mapping, security, configuration, state, and runtime boundaries.
3. Design and accept RFC-0002 / Bundle Format v1 separately.
4. Keep `luna-boot` and boot-state implementation separate from normal userspace crates.
