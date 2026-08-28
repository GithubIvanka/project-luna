# Phase 1.6 — Implementation Transition Record

**Status:** ACTIVE FOLLOW-THROUGH
**Source of Truth:** `docs/ARCHITECTURE.md`
**Related phase:** `docs/phases/PHASE-1.6.md`

This record documents repository work performed after the Phase 1.6 decision ledger was accepted through 1.6-HZ.

## Completed — repository transition

- Audited the real `main` workspace baseline.
- Removed dependence on the historical crate layout as an architectural source.
- Kept `luna-common` as the surviving foundation and treated its old API as subject to redesign.
- Added the architecture-defined scaffold boundaries required for the implementation transition.
- Kept `luna-boot` / `luna-boot.efi` outside the normal userspace workspace until its boot-specific target and API are designed.
- Kept `luna-log` outside the workspace until a concrete ownership/API requirement exists.
- Added `docs/architecture/CRATE-MAP.md` and the development API-contract record.
- Updated `README.md`, `STATUS.md`, and `ROADMAP.md` to reflect the current transition stage.

## Completed — foundation/domain API pass

### `luna-common`

Retained and refined:

- `BundleId`
- `ComponentId`
- `UserId`
- `Version`

Removed the old global `LunaError` / `LunaResult` model so subsystem-specific failures remain local to their owners.

### `luna-fs`

Implemented the initial low-level filesystem boundary:

- `FsError`
- `FileHandle`
- `FileMetadata`
- `FileSystem`
- `HostFileSystem`

This layer intentionally contains no authorization, logical-root policy, configuration, bundle or application policy.

### `luna-root-mapping`

Implemented the first per-namespace exact-file mapping contract:

- `LogicalPath`
- `PhysicalPath`
- `MappingRule`
- `MappingTable`
- `MappingError`

Directory-wide implicit mappings are not part of this first contract. The design explicitly supports mappings such as `/bin/app` → a resource inside an installed bundle.

### `luna-config`

Implemented:

- system/user/application scopes;
- `ConfigKey`;
- `ConfigValue`;
- `ConfigStore`;
- `MemoryConfigStore`;
- `LayeredConfig`;
- application lookup precedence.

Application lookup precedence is user/application override → application default → system default.

### `luna-security`

Implemented the central policy boundary:

- `Principal`;
- `Resource`;
- `Permission`;
- `AuthorizationRequest`;
- `Decision`;
- `PolicyAuthority`.

No root/sudo user abstraction is introduced.

### `luna-state`

Implemented the generic durable state contract:

- `StateKey`;
- `StateValue`;
- `StateError`;
- `StateStore`.

Specialized boot state remains outside this generic crate.

### `luna-event`

Implemented the event-domain boundary:

- `EventType`;
- `Event`;
- `EventPublisher`;
- `EventSubscriber`;
- `Subscription`.

No Kafka dependency or broker implementation is committed. The event contract may later be backed by Tokio-based infrastructure.

### `luna-bundle`

Implemented the internal bundle domain model:

- `BundleKind`;
- `BundleMetadata`;
- `BundleResource`;
- `BundleManifest`;
- `BundleError`;
- structural manifest validation.

`.lbp` remains a separate transport/archive format and RFC-0002 remains unaccepted.

### `luna-user-session`

Implemented:

- `SessionId`;
- `SessionState`;
- `UserSession`;
- lifecycle transition validation.

The session is a Luna domain instance rather than a Linux TTY abstraction.

## Current dependency direction

```text
luna-common
├── luna-config
├── luna-security
├── luna-event
├── luna-bundle
└── luna-user-session

luna-common ← luna-fs ← luna-root-mapping

luna-state is intentionally independent of higher-level policy.
```

Higher-level managers and runtimes consume these contracts rather than moving their responsibilities downward.

## Explicitly deferred

- Bundle Format v1 / `.lbp` wire/archive details until RFC-0002 is designed and accepted.
- System Image specification and manifest implementation.
- Kernel metadata specification.
- Boot-state metadata implementation.
- `luna-boot.efi` implementation.
- Final IPC transport.
- Final async channel/broker implementation.
- Final Linux namespace/materialization implementation.
- Final persistence backends.
- Final GUI implementation.

## Verification status

The repository was updated through this API/domain pass. A local `cargo test --workspace` could not be executed from the connected environment because the execution container could not resolve `github.com`. The code was therefore reviewed structurally, but no successful local compilation claim is made here.

## Next stage

The next stage is the **manager/runtime API pass**:

1. `luna-system-manager`;
2. `luna-kernel-manager`;
3. `luna-update-manager`;
4. `luna-app-manager`;
5. `luna-device-manager`;
6. `luna-system-runtime`;
7. `luna-app-runtime`;
8. `luna-cli`.

Their contracts must be derived from the foundation/domain APIs already established here.
