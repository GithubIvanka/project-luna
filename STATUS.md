# Project Luna — Status

Last updated: 2026-08-27

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ**. The project is now performing the repository/crate audit required before implementation.

### Phase status

| Phase | Status |
|---|---|
| 1.1 | Accepted and consolidated |
| 1.2 | Accepted and consolidated |
| 1.3 | Accepted and consolidated |
| 1.4 | Accepted and consolidated |
| 1.5 | Accepted and consolidated |
| 1.6 | Accepted through HZ and consolidated |
| Repository/crate audit | **Current** |

## Actual repository state

The `main` branch currently contains a minimal Rust workspace:

```text
project-luna/
├── Cargo.toml
├── components/
│   └── luna-common/
│       ├── Cargo.toml
│       └── src/
│           ├── id.rs
│           ├── lib.rs
│           └── version.rs
└── docs/
```

The old empty component crates were intentionally removed. This is correct: historical names are not implementation commitments.

## Repository audit result

- Root workspace: `resolver = "3"`, only `luna-common` remains.
- `luna-common` is useful as a foundation, but its old API was **not** accepted as the final API.
- The generic `LunaError` / `LunaResult` layer was removed because subsystem-specific errors belong to their owning crates.
- `BundleId` and `ComponentId` remain as opaque foundational value types.
- `Version` remains as a small foundational value type.
- No new subsystem crate was created merely to reserve a historical name.

These changes were applied directly to the repository as part of the Phase 1.6 audit.

## Architectural subsystems not yet created as crates

These names are architectural responsibility boundaries, not a list of empty crates to create immediately:

- `luna-cli`
- `luna-system-manager`
- `luna-app-manager`
- `luna-device-manager`
- `luna-update-manager`
- `luna-kernel-manager`
- `luna-root-mapping`
- `luna-security`
- `luna-system-runtime`
- `luna-app-runtime`
- `luna-fs`
- `luna-bundle`
- `luna-config`
- `luna-log`
- `luna-common`

Some responsibilities may later require a library + daemon/service + thin client split. The final workspace is derived from API boundaries, not from this list mechanically.

## Not implemented

The following are architecture/specification work, not implemented subsystems:

- `luna-boot.efi`
- System Image lifecycle
- kernel lifecycle/compatibility implementation
- Bundle Format v1 implementation
- application runtime
- system runtime
- security backend
- root mapping backend
- device manager
- update manager
- application manager
- GUI/CLI backend services

## Immediate next work

1. Derive the concrete crate map from `docs/ARCHITECTURE.md`.
2. Define responsibility, dependency, persistence and API boundaries.
3. Decide which boundaries need library, daemon/service, binary, or client crates.
4. Start the first implementation crate only after its contract is explicit.
5. Treat RFC-0002 as a separate design task; no proposed `.lbp` layout is accepted automatically.
