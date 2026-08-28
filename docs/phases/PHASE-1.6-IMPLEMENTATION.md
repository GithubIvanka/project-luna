# Phase 1.6 — Implementation Transition Record

**Status:** ACTIVE FOLLOW-THROUGH
**Source of Truth:** `docs/ARCHITECTURE.md`
**Related phase:** `docs/phases/PHASE-1.6.md`

This record documents repository work performed after the Phase 1.6 decision ledger was accepted through 1.6-HZ.

## Completed

- Audited the real `main` workspace baseline.
- Kept `luna-common` as the surviving historical foundation and treated its API as subject to redesign.
- Audited and retained the useful small `luna-common` value types (`BundleId`, `ComponentId`, `Version`).
- Added `luna-fs` as the low-level filesystem crate.
- Added `luna-root-mapping` as a separate logical mapping layer.
- Added `luna-config` as a configuration boundary.
- Added `luna-security` as the central policy boundary.
- Added `luna-state` as the persistent-state boundary.
- Added `luna-event` as the asynchronous event-contract boundary.
- Added `luna-bundle` as a bundle-domain boundary; Bundle Format v1 remains deferred to RFC-0002.
- Added `luna-app-manager` as bin + lib.
- Added `luna-system-manager` as bin + lib.
- Added `luna-update-manager` as bin + lib.
- Added `luna-device-manager` as bin + lib.
- Added `luna-kernel-manager` as bin + lib.
- Added `luna-system-runtime` as bin + lib.
- Added `luna-user-session` as the UserSession domain boundary.
- Added `luna-app-runtime` as bin + lib.
- Added `luna-cli` as a thin bin + lib client.
- Updated the workspace membership to include the architecture-defined scaffold boundaries that are currently implementation-ready.
- Kept bootloader and boot-state boundaries separate from normal userspace crates; their target/API design remains deferred.
- Added `docs/architecture/CRATE-MAP.md` as the explicit crate-boundary map.
- Updated `STATUS.md` and `README.md` to reflect the current repository state.

## Explicitly deferred

- Bundle Format v1 / `.lbp` wire and archive details until RFC-0002 is designed and accepted.
- `luna-boot.efi` implementation until its boot-specific target and API boundary are designed.
- `luna-boot-state` implementation until the early-boot metadata contract is specified.
- `luna-log` implementation until a concrete ownership/API requirement exists.
- Final GUI crate structure.

## Important implementation rule

The newly created crates are architectural scaffolds/boundary implementations, not completed subsystems. Their marker types must not be mistaken for finished functionality.

The next task is API-contract design, starting with the lowest-level boundaries and moving upward. A higher-level crate must not pull responsibilities downward merely to make the scaffold compile.
