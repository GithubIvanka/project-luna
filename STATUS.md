# Project Luna — Status

Last updated: 2026-08-27

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ**. The repository has now moved from audit into the first architecture-driven crate scaffolding.

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
| Crate map | Established |
| API contracts | **Next** |

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
│   ├── luna-app-manager/
│   ├── luna-system-manager/
│   ├── luna-update-manager/
│   ├── luna-device-manager/
│   ├── luna-kernel-manager/
│   ├── luna-system-runtime/
│   ├── luna-app-runtime/
│   └── luna-cli/
└── docs/
```

The crates created in this step are **scaffolds only**. They establish ownership boundaries; they do not claim completed subsystem implementations.

## Foundation audit

`luna-common` remains intentionally small. Its existing useful value types were retained; subsystem-specific errors and behaviour do not belong there.

`luna-fs` is the low-level filesystem layer.

`luna-root-mapping` is a separate logical mapping layer and does not own security policy.

## Management boundaries

- `luna-app-manager`: installation, update, removal, verification, migrations and package import; it does not own normal application execution.
- `luna-system-manager`: owns the system state model and queries.
- `luna-update-manager`: executes changes.
- `luna-kernel-manager`: kernel inventory/metadata/compatibility model; change execution remains with update-manager.
- `luna-device-manager`: device and volume lifecycle.
- `luna-security`: central policy authority.

## Runtime boundaries

- `luna-system-runtime`: single system-wide runtime supervising UserSessions.
- `luna-app-runtime`: application execution and ApplicationInstance lifecycle inside a UserSession.

## User interface

`luna-cli` is a thin client. Backend work is not duplicated in CLI code. Future GUI clients should use the same backend contracts.

## Deliberately deferred

- `luna-bundle` implementation: deferred until RFC-0002 / Bundle Format v1 is designed and accepted.
- `luna-boot.efi`: separate bootloader implementation boundary.
- `luna-log`: not created merely because of historical naming; create when a real ownership boundary requires it.

## Async direction

Phase 1.6 accepted Tokio as the Rust async-runtime direction where an async runtime is actually required. Lower-level crates must not acquire Tokio merely by association.

## Next work

1. Define API contracts for the foundation crates.
2. Audit/refine the scaffolded public APIs against `docs/ARCHITECTURE.md`.
3. Define persistence and error boundaries.
4. Implement the first real crate only after its contract is explicit.
5. Keep RFC-0002 as a separate design task.
