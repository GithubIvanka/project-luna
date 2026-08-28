# Phase 1.6 — Architecture Consolidation, Repository Reset & Crate Planning

**Status:** ACCEPTED / CONSOLIDATED
**Phase:** 1.6
**Project:** Project Luna
**Source of Truth:** `docs/ARCHITECTURE.md`
**Record purpose:** Preserve accepted Phase 1.6 decisions independently from chat history.

## 1. Purpose

Phase 1.6 establishes the repository/crate boundaries and the first implementation contracts after the architecture decision cycle. Historical empty crates are not architectural commitments. Existing code is retained only when it remains useful to the current contract.

## 2. Decision record

Decisions 1.6-A through 1.6-HZ were accepted. Explicit selections include:

```text
1.6-K  — B
1.6-AA — B
1.6-Db — Bin + lib
1.6-Di — Tokio
1.6-Ea — C
```

The complete A–HZ acceptance ledger is preserved in the earlier section of this file.

## 3. Repository policy

The repository reflects the current architecture rather than historical component names. Empty future crates are not created merely to reserve names. A component is added when its responsibility and API boundary are ready for implementation.

`luna-common` remains a small foundation crate. Subsystem-specific errors, policies, runtime state, filesystem operations, bundle semantics and service APIs belong to their owners.

Where a component needs both reusable library functionality and a process/service boundary, the accepted default is a small daemon/service plus library, represented in Cargo as **bin + lib** where appropriate.

## 4. Async policy

Tokio is the selected async runtime direction where an async runtime is actually required. This does not make Tokio a dependency of every crate.

## 5. API and integration decisions: 1.6-1 → 1.6-40

### 1.6-1
Logical paths use two stages:

```text
lexical normalization
        ↓
secure physical resolution
```

Redundant separators and `.` components may be normalized. `..` cannot escape the authorized logical root. Physical resolution and symlink traversal must be checked against the authorized boundary.

### 1.6-2
Mapping fallback is semantic-class-specific. There is no universal filesystem search chain. Only explicitly permitted resource classes may use user/application/system fallback.

### 1.6-3
Security application identity is based on `BundleId`. Version is additional context rather than a mandatory separate security principal.

### 1.6-4
Different versions of the same application are independent immutable runtime resources. Their instances, mappings and lifecycle are independent.

### 1.6-5
Luna uses a complete event stack:

```text
Event
 ↓
Event Bus
 ↓
Persistent/Event History where appropriate
 ↓
Subscribers / Replay
```

The model supports live delivery, persistent history for important events, replay/query, per-operation ordering, diagnostic/security/audit classes and event-class-specific retention. Kafka is only a conceptual reference.

### 1.6-6
`luna-state` represents logical persistent state. Checkpoint/rollback is a separate mechanism. Btrfs snapshots are an accepted possible implementation of checkpoints and rollback; `luna-state` is not Btrfs-specific.

### 1.6-7
`luna-app-manager` produces an `ApplicationPlan` describing target Bundle/version, dependency/resource requirements, migrations and requested application changes without embedding low-level mount/syscall details. `luna-update-manager` executes the resulting changes.

### 1.6-8
`luna-system-runtime` creates and supervises `UserSession` instances. No separate Session Manager exists solely for session creation.

### 1.6-9
`luna-system-runtime` is the system-wide authority for `ApplicationInstanceId` uniqueness.

### 1.6-10
Resource protection is part of the current implementation boundary. Resource budgets, limits and reservations use Linux resource-control mechanisms initially.

### 1.6-11
The first application-isolation implementation uses:

```text
Linux mount namespace
+
controlled bind mounts
+
Root Mapping
+
OverlayFS where a writable/COW layer is actually required
```

A custom virtual filesystem is deferred to a possible future major Luna version.

### 1.6-12
The state domains remain distinct:

```text
System State   → persistent normal-system state
Boot State     → bootloader-side persistent metadata
Recovery State → RAM-only temporary state
```

Recovery state does not become normal persistent UserSession state.

### 1.6-13
Logical paths are Linux-style absolute paths.

### 1.6-14
Physical paths are implementation-side and are not exposed to applications. The file manager exposes Luna's DATA structure and friendly external volumes.

### 1.6-15
File-level mappings are the default. Explicit whole-directory/subtree mappings are allowed when required by a semantic mapping class, such as a library subtree.

### 1.6-16
Each application namespace has its own RAM-resident mapping table. There is no single global mapping table for all applications.

### 1.6-17
Identical immutable mapping definitions may be shared across ApplicationInstances when semantically safe; namespace-specific state remains isolated.

### 1.6-18
Different applications may map the same logical dependency path to different physical dependency versions without conflict.

### 1.6-19
Conflicting mappings within one namespace are errors, not silent overrides.

### 1.6-20
System resources/defaults are the lowest precedence where fallback is permitted.

### 1.6-21
Application resources may shadow system defaults where the mapping class permits it.

### 1.6-22
User overrides may shadow application/default content where permitted.

### 1.6-23
User → application → system precedence is class-specific, not universal.

### 1.6-24
Security policy is evaluated before the final runtime mapping plan is accepted.

### 1.6-25
An application cannot rewrite its own MappingTable after namespace creation.

### 1.6-26
`luna-app-runtime` consumes a validated MappingPlan and does not invent mappings itself.

### 1.6-27
`luna-root-mapping` does not own application lifecycle semantics.

### 1.6-28
`ApplicationPlan` belongs to `luna-app-manager`.

### 1.6-29
`UpdatePlan` belongs to `luna-update-manager`.

### 1.6-30
Application plans reference requested resources/changes but do not contain low-level mount/syscall implementation details.

### 1.6-31
One high-level update operation may contain multiple application/system targets and their sub-statuses where transactional semantics permit it.

### 1.6-32
Each manager remains the owner of its own domain state even when several plans participate in a higher-level operation.

### 1.6-33
`ApplicationInstanceId` uniqueness is authoritative at system-runtime level.

### 1.6-34
`luna-system-runtime` is the global uniqueness authority, not individual per-user app-runtimes.

### 1.6-35
`luna-app-runtime` owns the lifecycle of an individual ApplicationInstance after creation.

### 1.6-36
The hierarchy is explicitly:

```text
luna-system-runtime
        ↓
UserSession
        ↓
luna-app-runtime
```

### 1.6-37
A `UserSession` contains user identity, session state and its session resource/policy context.

### 1.6-38
`luna-app-runtime` does not create UserSessions. It exists under the UserSession/system-runtime hierarchy.

### 1.6-39
Recovery uses a temporary recovery identity and does not create/reuse a normal persistent UserSession for the real user.

### 1.6-40
Recovery has RAM-backed state and explicit authorization before protected user data can be opened.

## 6. Integration decisions: 1.6-41 → 1.6-75

### 1.6-41
`luna-fs` owns low-level filesystem path primitives; it does not use `LogicalPath` as its public domain path type.

### 1.6-42
`PhysicalPath` belongs to the mapping/storage layer, not `luna-common`.

### 1.6-43
`luna-fs` supports files, directories, metadata, symlinks and filesystem mode/permission metadata without deciding authorization.

### 1.6-44
Filesystem failures use errors owned by `luna-fs`; there is no global `LunaError` in `luna-common`.

### 1.6-45
`luna-root-mapping` exposes typed logical/physical path concepts at its domain boundary.

### 1.6-46
`MappingTable` supports insertion, removal, lookup, conflict detection and validation.

### 1.6-47
A validated mapping table is immutable after handoff to runtime; changes produce a new table/version.

### 1.6-48
The immutable mapping-table model permits atomic replacement of mapping state.

### 1.6-49
`MappingPlan` is immutable after validation.

### 1.6-50
A mapping plan invalidated by a later Security policy change requires revalidation before reuse.

### 1.6-51
Security policy is revisioned and requests may refer to the policy revision/snapshot used for evaluation.

### 1.6-52
Grants may be operation-scoped, one-time, while-running or persistent where appropriate; an earlier successful request does not grant indefinite permission.

### 1.6-53
`luna-security` owns policy/grants/trust, not runtime process state.

### 1.6-54
Trust records bind at least `BundleId`, content identity/hash and trust scope.

### 1.6-55
Trust is content-specific; trusting one modified Bundle does not trust every future content under the same application identity.

### 1.6-56
Bundle resources are typed domain objects containing resource identity, logical path, bundle-relative source path and resource metadata/type.

### 1.6-57
Bundle manifests separate identity, metadata, resources, dependencies, capabilities and entry points; runtime state is excluded.

### 1.6-58
`ApplicationInstanceId` never belongs in the Bundle manifest.

### 1.6-59
`UserSessionId` never belongs in the Bundle manifest.

### 1.6-60
An immutable Bundle can be used from different UserSessions without modification.

### 1.6-61
`luna-bundle` does not own the physical installation path.

### 1.6-62
`luna-app-manager` constructs/validates `ApplicationPlan` without launching the application.

### 1.6-63
ApplicationPlan validates dependencies, compatibility, security authorization, resources and migrations before execution.

### 1.6-64
An invalid plan cannot enter the mutation stage.

### 1.6-65
`UpdatePlan` belongs to `luna-update-manager`, which executes accepted plans without redefining manager-owned semantics.

### 1.6-66
Update transaction stages are `prepare → checkpoint → apply → verify → commit`.

### 1.6-67
Old state remains authoritative before commit where the operation can guarantee it.

### 1.6-68
After interruption, reconciliation determines committed/partially committed/not committed state; failure does not imply automatic rollback.

### 1.6-69
Rollback is an explicit recovery/transaction action, not an automatic response to every crash.

### 1.6-70
`luna-state` may reference operations/checkpoints but does not own snapshot internals.

### 1.6-71
Events carry correlation metadata sufficient to associate them with operations and source/target context.

### 1.6-72
Live subscriptions are separate from persistent event history.

### 1.6-73
Event subscriptions have an explicit lifecycle.

### 1.6-74
GUI/CLI disconnect does not cancel backend operations or erase queryable operation state.

### 1.6-75
**Option C accepted:** `luna-state` contains the state domain plus storage traits/backend implementations; checkpoint/snapshot mechanisms remain separate and the state model remains storage-implementation agnostic.

## 7. Integration/API decisions: 1.6-76 → 1.6-115

### 1.6-76
`luna-state` uses a synchronous storage abstraction. Async orchestration belongs to higher layers.

### 1.6-77
State storage supports a minimal transaction abstraction for atomic groups of state changes.

### 1.6-78
State changes use revision-based optimistic concurrency.

### 1.6-79
`EventId` and `OperationId` are independent identities.

### 1.6-80
Event ordering is monotonic per operation; a global total order across independent operations is not required.

### 1.6-81
Event timestamps are metadata and are not the ordering mechanism.

### 1.6-82
Events use `Ephemeral`, `Persistent` and `Audit` classes with separate retention/persistence semantics.

### 1.6-83
Persistent event history supports replay/query by event and operation context.

### 1.6-84
Event delivery uses bounded queues/backpressure.

### 1.6-85
Overflow handling is class-specific; Audit events must not be silently dropped.

### 1.6-86
Interrupted operations are reconciled after responsible service/runtime recovery.

### 1.6-87
Operations distinguish resumable, non-resumable and unknown resumability where necessary.

### 1.6-88
Operations belong to System or UserSession context, never to GUI/CLI processes.

### 1.6-89
Operation authorization distinguishes view, cancel, resume and rollback.

### 1.6-90
Force Stop is distinct from cooperative Cancel and requires stronger/emergency authorization with warning where applicable.

### 1.6-91
Bundle logical paths use a dedicated validated domain type.

### 1.6-92
Bundle-relative source paths use a dedicated domain type.

### 1.6-93
Bundle resources carry explicit resource types/metadata.

### 1.6-94
Conflicting mappings of the same logical path inside one namespace are errors.

### 1.6-95
Exact duplicate resource entries may be deduplicated; different resources targeting one logical path remain a conflict.

### 1.6-96
Bundle identity is layered as `BundleId + Version + ContentIdentity`.

### 1.6-97
ContentIdentity is independent of filename and physical storage location.

### 1.6-98
Moving a Bundle does not change its identity.

### 1.6-99
External Bundles follow inspect → verify → trust decision → launch; no implicit plug-and-execute.

### 1.6-100
Permissions distinguish Visibility, Read, Write, Execute, DeviceUse and Manage.

### 1.6-101
Application-level restrictions propagate to all instances of that application identity.

### 1.6-102
An instance may be stricter than application policy, but never weaker.

### 1.6-103
Security policy is revisioned and relevant running/validated contexts may require revalidation after policy changes.

### 1.6-104
`Ask` means explicit user/system confirmation is required; Security itself remains UI-agnostic.

### 1.6-105
`Constrained` returns structured typed restrictions rather than free-form text.

### 1.6-106
ApplicationInstance lifecycle distinguishes Created, Starting, Running, Stopping, Stopped, Crashed and Failed.

### 1.6-107
`luna-system-runtime` detects and may restart failed app-runtime without automatically terminating the UserSession.

### 1.6-108
Runtime metadata recovery does not imply restoration of application process memory/state.

### 1.6-109
System-runtime restart is preferred over unnecessary full-machine reboot where recovery is possible.

### 1.6-110
A protected resource budget is reserved for system-critical services.

### 1.6-111
Resource reservation is adaptive rather than one universal fixed percentage.

### 1.6-112
Memory-pressure reclamation follows an ordered policy from disposable/reclaimable resources toward application pressure and termination, while system-critical memory remains protected.

### 1.6-113
GUI and CLI use shared backend contracts and do not operate directly on filesystem/runtime internals.

### 1.6-114
`luna` supports machine-readable output in addition to human-readable output; exact syntax remains a CLI specification concern.

### 1.6-115
Public internal component contracts have explicit compatibility versions; breaking changes require major API changes and incompatible clients are rejected explicitly.

## 8. Implementation boundary

The implementation sequence is:

```text
Architecture
    ↓
Repository / Cargo audit
    ↓
Crate map
    ↓
Foundation/domain contracts
    ↓
Manager/runtime contracts
    ↓
Integration tests
    ↓
Prototype
    ↓
Implementation
```

`docs/ARCHITECTURE.md` remains the single architectural Source of Truth. This phase file preserves the decision history and implementation-transition record.

`RFC-0002` remains a separate design task. Its current `.lbp` proposal is not automatically accepted.

## 9. Rejected audit proposals

The following proposals from the external code audit were explicitly rejected or corrected:

1. `resolver = "3"` → `resolver = "2"` — rejected. Resolver 3 is correct for the Rust 2024 direction.
2. `SystemState.previous` as the sole System Image fallback model — rejected. System Image fallback uses inventory and compatibility queries and is not limited to one previous image.
3. `BundleResource` raw strings → host `PathBuf` as a universal domain representation — rejected. Bundle uses dedicated logical/source path domain types.
4. Mandatory `into_string()` on every wrapper — rejected as a general architectural rule.
5. Mandatory `const fn` wherever technically possible — rejected as an architecture/style mandate.

## 10. Phase status

```text
Phase 1.1 — accepted and consolidated
Phase 1.2 — accepted and consolidated
Phase 1.3 — accepted and consolidated
Phase 1.4 — accepted and consolidated
Phase 1.5 — accepted and consolidated
Phase 1.6 — accepted through 1.6-115 and HZ ledger preserved
```

`docs/ARCHITECTURE.md` remains the single architectural Source of Truth.
