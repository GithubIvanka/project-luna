# Phase 1.6 — Implementation Transition Record

**Status:** ACTIVE FOLLOW-THROUGH
**Source of Truth:** `docs/ARCHITECTURE.md`
**Related phase:** `docs/phases/PHASE-1.6.md`

This record documents the repository work performed after the Phase 1.6 decision ledger was accepted through 1.6-HZ.

## Completed

- Audited the real `main` workspace baseline.
- Kept `luna-common` as the surviving historical foundation and treated its API as subject to redesign.
- Added `luna-fs` as the low-level filesystem crate.
- Added `luna-root-mapping` as a separate logical mapping layer.
- Added `luna-config` as a configuration boundary.
- Added `luna-security` as the central policy boundary.
- Added `luna-app-manager` as bin + lib.
- Added `luna-system-manager` as bin + lib.
- Added `luna-update-manager` as bin + lib.
- Added `luna-device-manager` as bin + lib.
- Added `luna-kernel-manager` as bin + lib.
- Added `luna-system-runtime` as bin + lib.
- Added `luna-app-runtime` as bin + lib.
- Added `luna-cli` as a thin bin + lib client.
- Updated the workspace membership accordingly.
- Added `docs/architecture/CRATE-MAP.md`.
- Updated `STATUS.md` and `ROADMAP.md` to reflect the real repository state.

## Explicitly deferred

- `luna-bundle` implementation until RFC-0002 / Bundle Format v1 is designed.
- `luna-boot.efi` implementation until its boot-specific target and API boundary are designed.
- `luna-log` as a historical component name without a current ownership requirement.

## Important implementation rule

The newly created crates are architectural scaffolds, not completed APIs. Their empty boundary types must not be mistaken for finished subsystem implementations.

The next task is API-contract design, starting with the lowest-level boundaries and moving upward. A higher-level crate must not pull responsibilities downward merely to make the scaffold compile.
