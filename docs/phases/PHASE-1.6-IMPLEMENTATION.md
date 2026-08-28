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
- Added `docs/architecture/CRATE-MAP.md` and the development API-contract records.
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

Implemented the per-namespace logical mapping contract:

- `LogicalPath`
- `PhysicalPath`
- `MappingRule`
- `MappingTable`
- `MappingError`

Individual file mappings are the normal case. Explicit whole-directory/subtree mappings are allowed for an appropriate semantic mapping class, such as a library subtree. They are not an unrestricted implicit mapping mechanism.

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

Implemented the generic durable state contract and the first concurrency hardening pass:

- `StateKey`;
- `StateValue`;
- `Revision`;
- `StateMutation`;
- `StateTransaction`;
- `StateError` with revision conflict reporting;
- `StateStore::revision`;
- atomic `StateStore::transaction` boundary.

The state abstraction remains synchronous. Higher layers own asynchronous orchestration and synchronization.

The accepted Phase 1.6 concurrency model is a global store revision. A transaction commits only when its expected revision matches the current revision. A stale transaction is rejected without partial writes. A successful non-empty transaction advances the revision once; an empty transaction leaves it unchanged.

The contract does not select the eventual persistence backend, WAL, filesystem format, or snapshot implementation.

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

## Completed — manager/runtime API baseline

### `luna-system-manager`

Implemented `SystemImageRef`, `SystemState`, and `SystemQuery`. `current` and immutable `factory` System Image state are kept separate.

### `luna-kernel-manager`

Implemented `KernelRef`, `KernelSelection`, and `KernelQuery`. The model explicitly contains current, previous, and immutable factory kernel identities so Factory remains an image+kernel recovery point.

### `luna-update-manager`

Implemented `UpdateOperation`, `UpdatePlan`, `UpdateError`, and `UpdateExecutor`. The crate is the mutation/execution side and does not become the owner of desired state.

### `luna-app-manager`

Implemented `ApplicationRef`, `ApplicationOperation`, `AppManagerError`, and `ApplicationManager`. It models lifecycle work and does not own normal execution.

### `luna-device-manager`

Implemented device/volume query concepts and explicit volume lifecycle states. Security and mount policy remain outside the crate.

### `luna-system-runtime`

Implemented the system-wide runtime boundary and explicit dependency on event/session contracts. One system runtime supervises multiple UserSessions.

### `luna-app-runtime`

Implemented `ApplicationInstanceId`, `InstanceState`, `ApplicationInstance`, and `ApplicationRuntime`. The runtime integrates bundle, mapping, session, and security contracts without taking over application installation/update/removal.

### `luna-cli`

Remains a thin client. Its final command grammar and backend transport remain intentionally unfrozen.

## Integration testing

Added a cross-domain contract test under `luna-app-runtime/tests/contracts.rs` covering composition of bundle metadata, logical root mapping, security authorization, and application runtime creation.

Added focused `luna-state` contract tests covering:

- basic round-trip persistence;
- atomic multi-mutation transactions;
- single revision advancement per non-empty transaction;
- stale-revision rejection without partial writes;
- empty transactions preserving the current revision.

## Current dependency direction

```text
luna-common
├── luna-config
├── luna-security
├── luna-event
├── luna-bundle
├── luna-user-session
└── manager/runtime consumers

luna-common ← luna-fs ← luna-root-mapping

luna-event + luna-user-session → luna-system-runtime
luna-bundle + luna-root-mapping + luna-security + luna-user-session → luna-app-runtime
```

Higher-level managers and runtimes consume these contracts rather than moving their responsibilities downward.

## Explicitly deferred

- Bundle Format v1 / `.lbp` wire/archive details until RFC-0002 is designed and accepted.
- System Image specification and manifest implementation.
- Kernel metadata specification and compatibility resolver implementation.
- Boot-state metadata implementation.
- `luna-boot.efi` implementation.
- Final IPC transport.
- Final async channel/broker implementation.
- Final Linux namespace/materialization implementation.
- Final persistence backends.
- Device automount backend.
- Permission enforcement implementation.
- Final CLI grammar and aliases.
- Final GUI implementation.
- `.deb` / `.rpm` conversion internals.
- Update transaction/checkpoint protocol.

## Verification status

The repository was updated through the foundation, domain, manager, runtime, and first integration-test passes. A local `cargo test --workspace` could not be executed from the connected environment because the execution container could not resolve `github.com`. The code was therefore reviewed structurally, but no successful local compilation claim is made here.

## Next stage

The next stage is **integration-contract hardening**, followed by focused high-risk prototypes:

1. mapping + security authorization;
2. configuration + user/application precedence;
3. bundle + mapping validation;
4. session + application-instance lifecycle;
5. manager state + update plans;
6. event delivery boundaries;
7. namespace/materialization prototype;
8. persistence/update transaction prototype;
9. separate RFC-0002 Bundle Format v1 design.
