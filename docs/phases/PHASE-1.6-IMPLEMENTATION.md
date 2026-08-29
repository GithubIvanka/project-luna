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

Retained and refined the small shared value types:

- `BundleId`;
- `ComponentId`;
- `UserId`;
- `Version`.

The old global `LunaError` / `LunaResult` model was removed so subsystem-specific failures remain local to their owners.

### `luna-fs`

Implemented the initial low-level filesystem boundary:

- `FsError`;
- `FileHandle`;
- `FileMetadata`;
- `FileSystem`;
- `HostFileSystem`.

No authorization, logical-root policy, configuration, bundle or application policy lives here.

### `luna-root-mapping`

Implemented the per-namespace logical mapping domain:

- `LogicalPath`;
- `PhysicalPath`;
- `MappingKind`;
- `MappingRule`;
- `MappingTable`;
- `MappingError`;
- deterministic logical namespace description.

Logical paths now undergo lexical normalization without host filesystem resolution. `..` traversal is rejected. Exact file mappings are the default, while explicit subtree mappings are supported for suitable semantic classes such as libraries. The crate contains no Linux namespace syscalls and no authorization policy.

### `luna-config`

Implemented:

- system/user/application scopes;
- `ConfigKey`;
- `ConfigValue`;
- `ConfigStore`;
- `MemoryConfigStore`;
- `LayeredConfig`;
- application lookup precedence.

Application precedence is user/application override → application default → system default where that semantic class permits fallback.

### `luna-security`

Implemented the central policy boundary:

- `Principal`;
- `Resource`;
- `Permission`;
- `AuthorizationRequest`;
- `Decision`;
- `PolicyAuthority`.

The decision model includes `Allow`, `Deny`, `Ask`, and structured `Constrained` results. No root/sudo user abstraction is introduced.

### `luna-state`

Implemented the synchronous state contract and first concurrency hardening pass:

- `StateKey`;
- `StateValue`;
- `Revision`;
- `StateMutation`;
- `StateTransaction`;
- revision conflict reporting;
- atomic transaction boundary.

The accepted model is a global store revision. A transaction commits only when its expected revision matches the current revision. Stale transactions are rejected without partial writes. The state crate remains backend-agnostic; checkpoint/snapshot internals remain separate.

### `luna-event`

Implemented the event domain boundary:

- `EventType`;
- `Event`;
- `EventPublisher`;
- `EventSubscriber`;
- `Subscription`.

Kafka remains only a conceptual analogy. The final transport/broker is not selected here.

### `luna-bundle`

Implemented the internal Bundle domain model:

- `BundleKind`;
- `BundleMetadata`;
- `BundleResource`;
- `BundleManifest`;
- `BundleError`;
- structural manifest validation.

Bundle identity is conceptually `BundleId + Version + ContentIdentity`. `.lbp` remains the transport/archive representation and RFC-0002 remains unaccepted until final format review.

### `luna-user-session`

Implemented:

- `SessionId`;
- `SessionState`;
- `UserSession`;
- lifecycle transition validation.

`UserSession` is the combined user/session domain entity; no separate Session Manager is introduced.

## Completed — manager/runtime API baseline

### `luna-system-manager`

Owns System Image state/query semantics. It does not execute updates.

### `luna-kernel-manager`

Owns kernel inventory/query/compatibility semantics. Current, previous and immutable factory kernel identities are distinct concepts; fallback is not reduced to a single `previous` slot for System Images.

### `luna-update-manager`

Owns execution of system/application changes. It does not become the owner of desired domain state.

### `luna-app-manager`

Owns install/update/removal/verification/migration/package-import planning. Normal application execution belongs to runtime.

### `luna-device-manager`

Owns device and volume discovery/lifecycle concepts. Permission and mount policy remain separate.

### `luna-system-runtime`

One system-wide runtime supervises multiple `UserSession`s and their application runtimes. There is no separate `lunad` architectural component.

### `luna-app-runtime`

Owns `ApplicationInstance` lifecycle and consumes validated bundle/mapping/security/session contracts. It does not own installation/update/removal policy.

### `luna-cli`

Remains a thin client over backend contracts. Human-readable and machine-readable output are both supported conceptually; final grammar remains a separate CLI concern.

## Namespace/materialization — first real Linux backend

A dedicated `luna-namespace` crate is now part of the workspace. It owns Linux-specific namespace/materialization primitives so `luna-root-mapping` stays a pure domain/mapping layer.

Current implementation provides:

- `unshare(CLONE_NEWNS)` for a private mount namespace;
- private mount propagation using `MS_PRIVATE | MS_REC`;
- controlled bind mounts from validated physical resources to logical destinations;
- read-only remounting of those bind mounts.

The backend deliberately assumes authorization has already succeeded. It does not implement Security policy, build the full logical `/` tree, or own application lifecycle.

Linux mount namespaces provide isolated mount views, and bind mounts provide the mechanism for exposing selected existing filesystem resources at controlled logical destinations. citeturn415334search2turn415334search6

ID-mapped mounts remain an optional later mechanism for localized UID/GID views. citeturn415334search1turn415334search4

## Accepted post-HZ implementation clarifications

The 2026-08-29 audit additionally fixed the following boundaries:

1. `luna-system-runtime` is the system-wide runtime/supervisor; do not introduce a second `lunad` concept.
2. `UserSession` combines the user/session identity.
3. The application sees a normal Linux-compatible logical `/`; it is not given an artificially truncated fake root.
4. The physical Luna DATA tree remains Luna-native and is not exposed verbatim as a hidden `/data` application root.
5. The canonical DATA structure is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

6. Mapping is file-based by default, but explicit directory/subtree mappings are valid where the semantic class calls for them.
7. Manifest-declared mappings are requests/description; final mapping and authorization are built by higher-level runtime/manager logic.
8. Security policy is central and instance restrictions may tighten but cannot weaken an enforced application/system denial.
9. Applications use filtered resource views for devices, D-Bus and other host services rather than receiving unrestricted host access.
10. `cgroups v2` and other existing Linux resource-control mechanisms are preferred for enforcement of the already accepted resource-protection policy.
11. `DATA/system/state` is the preferred home for persistent system state; a backend implementation may use an embedded transactional key/value store without becoming a new architectural boundary.
12. A boot-success marker and watchdog participate in update/fallback health determination; a kernel boot alone does not prove that the new system is healthy.
13. Recovery remains a separate Recovery System Image with temporary RAM-backed identity/state; it is not the Factory System Image and does not use a normal persistent user session.
14. `.lbp` remains an archive/transport representation of a Bundle; it is not the runtime representation and is not the System Image format.
15. `luna-bundle` owns Bundle domain/format concerns; `luna-app-manager` owns Bundle lifecycle management and package import.
16. A new generic `luna-runtime` crate is not introduced; `luna-system-runtime` and `luna-app-runtime` are the runtime boundaries.
17. No separate `luna-update` crate is introduced; `luna-update-manager` is the update execution component.
18. `dependi` is build/install/dependency tooling, not a runtime dependency resolver service.
19. Final `.lbp` and dependency format compatibility are deferred to RFC/specification work rather than inferred from current placeholder code.

## Integration testing

Cross-domain tests cover composition of bundle metadata, logical root mapping, security authorization, session state and application runtime creation.

`luna-state` tests cover:

- round-trip state access;
- atomic multi-mutation transactions;
- revision advancement;
- stale-revision rejection without partial writes;
- empty transactions preserving revision.

The Linux namespace backend currently has host-testable error behavior only; privileged namespace/mount execution is a dedicated integration-test target.

## Explicitly deferred

- complete logical-root construction and per-application mount layout;
- privileged helper separation and hardened syscall/FD lifecycle;
- `/proc`, `/sys`, `/dev`, `/run`, Wayland, PipeWire and D-Bus policy-specific materialization;
- user/PID/network/IPC/UTS namespace policy profiles;
- optional OverlayFS and idmapped-mount integration;
- durable `luna-state` backend and crash-consistency implementation;
- update/checkpoint/rollback engine;
- System Image manifest/compatibility implementation;
- persistent boot-state metadata;
- final IPC transport and async event transport;
- Bundle Format v1 / RFC-0002 acceptance and `.lbp` codec;
- production signature/trust chain, fs-verity/IMA/TPM integration decisions;
- device automount backend;
- final CLI/GUI implementation and resource policy tuning.

## Verification status

The connected GitHub environment allows repository inspection and edits but does not provide a trusted local Linux execution environment for every privileged namespace operation. CI remains the authoritative place to run workspace and Linux-backend tests that require the appropriate runner capabilities. No unverified local success is claimed.

## Next stage

The next high-risk implementation sequence is:

1. complete logical-root materialization on top of `luna-root-mapping` + `luna-namespace`;
2. durable `luna-state` storage and crash recovery;
3. update/checkpoint/rollback transaction engine;
4. RFC-0002 Bundle Format v1 design and acceptance;
5. bundle codec and package import integration;
6. production signature/trust/integrity chain.
