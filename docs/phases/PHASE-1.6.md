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

## 6. Implementation boundary

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
