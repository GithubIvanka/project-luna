# Project Luna — Status

Last updated: 2026-08-28

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ**. The repository is now aligned with the architecture-driven crate map at the scaffold level.

### Phase status

| Phase | Status |
|---|---|
| 1.1 | Accepted and consolidated |
| 1.2 | Accepted and consolidated |
| 1.3 | Accepted and consolidated |
| 1.4 | Accepted and consolidated |
| 1.5 | Accepted and consolidated |
| 1.6 | Accepted through HZ and consolidated |
| Repository/Cargo audit | Completed for current main baseline |
| Crate map | Established and workspace-aligned |
| API contracts | Current work |

## Current repository structure

```text
project-luna/
├── Cargo.toml
├── components/
│   ├── luna-common/
│   ├── luna-fs/
│   ├── luna-root-mapping/
│   ├── luna-config/
│   ├── luna-security/
│   ├── luna-state/
│   ├── luna-event/
│   ├── luna-bundle/
│   ├── luna-app-manager/
│   ├── luna-system-manager/
│   ├── luna-update-manager/
│   ├── luna-device-manager/
│   ├── luna-kernel-manager/
│   ├── luna-system-runtime/
│   ├── luna-user-session/
│   ├── luna-app-runtime/
│   └── luna-cli/
└── docs/
```

These crates are **scaffolds/boundary implementations only**. They do not claim completed subsystem functionality.

`luna-boot`/`luna-boot.efi` and `luna-log` remain intentionally outside the workspace until their implementation boundaries require them. A future GUI crate is also deferred.

## Foundation audit

`luna-common` remains intentionally small. Its useful value types were retained; subsystem-specific errors and behaviour do not belong there.

`luna-fs` is the low-level filesystem layer.

`luna-root-mapping` is a separate logical mapping layer and does not own security policy.

`luna-config` owns configuration representation and layered lookup/persistence contracts, not authorization.

`luna-state` is the durable state boundary; specialized boot metadata remains separate.

`luna-event` defines event contracts without committing Luna to Kafka or another broker.

`luna-bundle` is only the bundle-domain boundary for now. RFC-0002 still defines the future Bundle Format v1 details.

## Management boundaries

- `luna-app-manager`: installation, update, removal, verification, migrations and package import; it does not own normal application execution.
- `luna-system-manager`: owns the system state model and queries.
- `luna-update-manager`: executes changes.
- `luna-kernel-manager`: kernel inventory/metadata/compatibility model; change execution remains with update-manager.
- `luna-device-manager`: device and volume lifecycle.
- `luna-security`: central policy authority.

## Runtime boundaries

- `luna-system-runtime`: single system-wide runtime supervising UserSessions.
- `luna-user-session`: UserSession domain boundary.
- `luna-app-runtime`: application execution and ApplicationInstance lifecycle inside a UserSession.

## User interface

`luna-cli` is a thin client. Backend work is not duplicated in CLI code. Future GUI clients should use the same backend contracts.

## Async direction

Phase 1.6 accepted Tokio as the Rust async-runtime direction where an async runtime is actually required. Lower-level crates must not acquire Tokio merely by association.

## Next work

1. Finish and reconcile public API contracts for the foundation crates.
2. Audit the scaffolded APIs against `docs/ARCHITECTURE.md` and remove accidental implementation commitments.
3. Define persistence and error boundaries.
4. Implement the first real crate only after its contract is explicit.
5. Keep RFC-0002 as a separate design task; do not treat the current bundle scaffold as a Bundle Format v1 specification.
