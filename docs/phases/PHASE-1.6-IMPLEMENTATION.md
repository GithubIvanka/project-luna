# Phase 1.6 — Implementation Transition Record

**Status:** ACTIVE FOLLOW-THROUGH
**Source of Truth:** `docs/ARCHITECTURE.md`
**Related phase:** `docs/phases/PHASE-1.6.md`

This record documents repository work performed after the Phase 1.6 decision ledger was accepted through 1.6-HZ, plus the clarifications accepted during the 2026-08-29 architecture audit.

## Completed — repository transition

- Audited the real `main` workspace baseline.
- Removed dependence on the historical crate layout as an architectural source.
- Kept `luna-common` as the surviving foundation and treated its old API as subject to redesign.
- Added the architecture-defined scaffold boundaries required for the implementation transition.
- Kept `luna-boot.efi` outside the normal userspace workspace because its UEFI/early-boot boundary is distinct.
- Kept `luna-log` outside the workspace until a concrete ownership/API requirement exists.
- Added and synchronized `docs/architecture/CRATE-MAP.md`.
- Updated `README.md` and `STATUS.md` to reflect the actual implementation stage.

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

File mappings are the default case. Explicit subtree/directory mappings are allowed for suitable semantic classes such as shared library trees; they are never an unrestricted implicit overlay.

### `luna-config`

Implemented:

- system/user/application scopes;
- `ConfigKey`;
- `ConfigValue`;
- `ConfigStore`;
- `MemoryConfigStore`;
- `LayeredConfig`;
- application lookup precedence.

System-wide mutable configuration belongs under `DATA/system/config/`; user-specific configuration belongs under `DATA/users/<user>/config/`.

### `luna-security`

Implemented the central policy boundary:

- `Principal`;
- `Resource`;
- `Permission`;
- `AuthorizationRequest`;
- `Decision`;
- `PolicyAuthority`.

No permanent root/sudo user abstraction is introduced.

### `luna-state`

Implemented the generic durable-state contract and concurrency hardening:

- `StateKey`;
- `StateValue`;
- `Revision`;
- `StateMutation`;
- `StateTransaction`;
- revision conflict reporting;
- atomic transaction boundary.

The abstraction remains synchronous. Higher layers own asynchronous orchestration and synchronization. The accepted first implementation uses a global store revision: stale transactions fail atomically and successful non-empty transactions advance the revision once.

### `luna-event`

Implemented the event-domain boundary:

- `EventType`;
- `Event`;
- `EventPublisher`;
- `EventSubscriber`;
- `Subscription`.

No Kafka dependency is committed. Kafka is only a conceptual model where useful; the implementation can remain local and lightweight.

### `luna-bundle`

Implemented the internal Bundle domain model:

- `BundleKind`;
- `BundleMetadata`;
- `BundleResource`;
- `BundleManifest`;
- `BundleError`;
- structural manifest validation.

`.lbp` remains a transport/archive representation and is not yet an accepted final wire format.

### `luna-user-session`

Implemented:

- `SessionId`;
- `SessionState`;
- `UserSession`;
- lifecycle transition validation.

`UserSession` is the combined user/session domain entity. It is not a Linux TTY abstraction.

## Completed — manager/runtime API baseline

### `luna-system-manager`

Implemented `SystemImageRef`, `SystemState`, and `SystemQuery`.

### `luna-kernel-manager`

Implemented `KernelRef`, `KernelSelection`, and `KernelQuery`. Current, previous and immutable factory kernel identities are modeled separately.

### `luna-update-manager`

Implemented `UpdateOperation`, `UpdatePlan`, `UpdateError`, and `UpdateExecutor`. It is the mutation/execution side and does not become the owner of desired system state.

### `luna-app-manager`

Implemented `ApplicationRef`, `ApplicationOperation`, `AppManagerError`, and `ApplicationManager`. It manages lifecycle changes and package import but does not own normal application execution.

### `luna-device-manager`

Implemented device/volume query concepts and explicit volume lifecycle states. Security and mount policy remain outside this crate.

### `luna-system-runtime`

Implemented the system-wide runtime boundary. One `luna-system-runtime` supervises multiple `UserSession` instances and their app runtimes. There is no separate `lunad` component.

### `luna-app-runtime`

Implemented `ApplicationInstanceId`, `InstanceState`, `ApplicationInstance`, and `ApplicationRuntime`. It consumes validated bundle/mapping/session/security contracts and owns execution/lifecycle of application instances.

### `luna-cli`

Remains a thin client. Final command grammar and transport remain specification work.

## Integration hardening completed

- Cross-domain contract tests cover bundle + mapping + security + runtime composition.
- `luna-state` tests cover atomic transactions, global revisions, stale revision rejection and empty transactions.
- Namespace/materialization exists as a pure deterministic contract prototype; it does not yet perform kernel mounts.
- Update execution exists as an in-memory atomic prototype; it does not yet mutate real System Images/Bundles/checkpoints.

## Accepted post-HZ clarifications — 2026-08-29

The following points were explicitly accepted after re-checking the architecture against the repository and the previous phase history.

### Runtime and sessions

- `luna-system-runtime` is the single system-wide runtime/supervisor; no separate `lunad` architectural component is introduced.
- `UserSession` is the single combined user/session entity.
- Runtime hierarchy is `luna-system-runtime → UserSession → luna-app-runtime → ApplicationInstance`.
- `luna-app-manager` is not part of the normal execution chain.
- Application execution belongs to `luna-app-runtime`.

### Logical root and namespace

- Applications receive a conventional Linux-compatible logical `/`, not an artificial reduced container filesystem.
- The physical Luna `DATA` layout remains Luna-native and is composed into the logical root through mappings/materialization.
- Mapping tables remain namespace-specific and RAM-resident at runtime.
- File mapping is the default; explicit subtree mapping is permitted for suitable semantic resources such as libraries.
- Linux namespaces, bind mounts and related mechanisms are implementation primitives.
- Container-visible identity is not a goal; applications should not be designed around the assumption that they are running inside a container.
- Root semantics are not granted to ordinary applications merely because Linux user namespaces are used.
- `idmapped` mounts are allowed as an implementation primitive when they simplify safe user/namespace ownership handling; they are not a mandatory Luna abstraction.

### DATA and state

The canonical user-visible mutable layout is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/<user>/
│   ├── home/
│   ├── data/
│   └── config/
└── cache/
```

- `DATA/system/state/` is the canonical location for persistent system state that is not ordinary configuration.
- State ownership remains with the relevant system subsystem; applications do not gain direct filesystem-level access to the state store.
- `DATA/system/config/` remains for system-wide mutable configuration.

### Security and device views

- Security remains a separate central policy authority.
- Permission decisions are multi-level and may return Ask or structured constrained access.
- D-Bus access should use a filtered/limited interface rather than exposing a full unrestricted system bus to applications.
- `/dev` should be presented as a filtered device view containing only resources authorized for the application/session.
- USB and other external-device access follows device discovery → policy → authorized application visibility/access; no automatic arbitrary execution.

### Resource control

- Linux cgroups v2 and related kernel mechanisms are the initial enforcement primitives for CPU, memory and process/resource limits.
- A protected system-critical resource budget is reserved.
- Memory pressure reclamation remains globally controlled by the system rather than by the current interactive user.
- Process-count and file-descriptor limits are ordinary resource safeguards; they do not create new Luna architectural components.
- Disk usage limits may be enforced through the underlying filesystem/resource facilities when required.

### Persistent storage implementation direction

For the first durable `luna-state` backend, use a small embedded transactional key/value database rather than introducing a client/server database or a custom WAL layer. `redb` is the current implementation choice for the prototype/backend; it is an implementation decision and not an architectural invariant.

A separate Luna WAL is not required when the selected storage backend already provides the required atomic durability semantics.

### Operations and recovery

- A successful boot should eventually be confirmed by a userspace health/boot-success marker rather than treating kernel handoff alone as success.
- A watchdog/timeout mechanism may mark a boot attempt failed when the system does not reach its healthy state.
- Crash recovery remains event-driven and user-visible after repeated failures, with options such as diagnostics, restart, rollback or close according to policy.
- Recovery remains the dedicated Recovery System Image + temporary RAM-backed recovery identity model, not Factory and not a normal persistent user session.

### Bundle and distribution

- `luna-bundle` owns Bundle domain representation and format codec concerns.
- `luna-app-manager` owns install/update/remove/verify/migration/package-import lifecycle.
- No separate `.lbp` parser crate is introduced merely for parsing the archive format.
- `.lbp` remains transport/archive representation, while the installed Bundle remains the immutable runtime unit.
- Reproducible build information and artifact provenance are desirable implementation properties for the future signature/trust chain.

### Documentation/repository consistency

- Historical phase records preserve traceability but do not compete with `docs/ARCHITECTURE.md`.
- `docs/architecture/CRATE-MAP.md`, `README.md`, `STATUS.md` and this implementation record must describe repository reality and never invent components that do not exist.

## `luna-boot.efi` status

Bootloader implementation is maintained under `boot/luna-boot/`, outside the normal userspace workspace. The current parallel boot track has progressed beyond the original scaffold: kernel loading and a test init path have been demonstrated through to `sh`.

Production signature/trust integration, final compatibility policy, production boot-state persistence and remaining hardening are still separate tasks.

## Verification status

Repository changes are verified structurally through GitHub. GitHub Actions is configured for Rust formatting/check/test/clippy/build validation and the separate UEFI target. Local interactive Cargo execution is not claimed when the connected environment cannot run it.

## Next real backend work

1. Real Linux namespace/materialization backend.
2. Durable `luna-state` backend and crash-safe recovery.
3. Real update/checkpoint/rollback engine.
4. Final Bundle Format v1 and RFC-0002 acceptance.
5. Production signature/trust chain.
6. System Image and kernel compatibility specifications/implementations.
