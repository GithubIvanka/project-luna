# Phase 1.6 — Decisions 41–75

**Status:** ACCEPTED / CONSOLIDATED
**Source of Truth:** `docs/ARCHITECTURE.md`

This file preserves the accepted Phase 1.6 decisions 41–75 as traceability material. The authoritative architectural semantics live in `docs/ARCHITECTURE.md`.

## 41–50 — Filesystem and mapping contracts

**1.6-41 — ACCEPTED**

`luna-fs` provides low-level filesystem path primitives. It does not use `LogicalPath` as its public domain path type.

**1.6-42 — ACCEPTED**

`PhysicalPath` is owned by mapping/storage layers and is not promoted to a generic `luna-common` primitive.

**1.6-43 — ACCEPTED**

`luna-fs` supports the fundamental filesystem concepts required by upper layers, including files, directories, metadata, symlinks and filesystem mode/permission metadata, without deciding authorization policy.

**1.6-44 — ACCEPTED**

Filesystem failures use errors owned by `luna-fs`; there is no global `LunaError` enum in `luna-common`.

**1.6-45 — ACCEPTED**

`luna-root-mapping` exposes typed logical/physical path concepts at its domain boundary rather than exposing raw host filesystem paths directly to applications.

**1.6-46 — ACCEPTED**

`MappingTable` supports insertion, removal, lookup, conflict detection and validation.

**1.6-47 — ACCEPTED**

A validated mapping table is treated as immutable after it is handed to runtime. Changes produce a new table/version rather than mutating the active table in place.

**1.6-48 — ACCEPTED**

The immutable mapping-table model permits atomic replacement of mapping state.

**1.6-49 — ACCEPTED**

`MappingPlan` is immutable after validation.

**1.6-50 — ACCEPTED**

If security policy changes after a mapping plan is validated and the plan is no longer permitted, the old plan cannot be reused without revalidation.

## 51–55 — Security policy and trust

**1.6-51 — ACCEPTED**

Security policy is versioned/revisioned. Requests may refer to the policy revision/snapshot against which they were evaluated.

**1.6-52 — ACCEPTED**

Security grants may be time/scoped. Supported duration concepts include operation-scoped, one-time, while-running and persistent grants where appropriate. A grant must not be treated as valid forever merely because an earlier request succeeded.

**1.6-53 — ACCEPTED**

`luna-security` owns policy/grants/trust but not runtime process state.

**1.6-54 — ACCEPTED**

Trust records are bound to at least the application identity (`BundleId`), content identity/hash and trust scope.

**1.6-55 — ACCEPTED**

Trust is content-specific. Trusting one modified Bundle does not automatically trust every future content version carrying the same application identity.

## 56–61 — Bundle resource/domain boundary

**1.6-56 — ACCEPTED**

Bundle resources are typed domain objects describing resource identity, logical path, bundle-relative source path and resource type/metadata.

**1.6-57 — ACCEPTED**

Bundle manifests conceptually separate identity, metadata, resources, dependencies, capabilities and entry points. Runtime-only identities/state do not belong in the Bundle manifest.

**1.6-58 — ACCEPTED**

`ApplicationInstanceId` never belongs to a Bundle manifest.

**1.6-59 — ACCEPTED**

`UserSessionId` never belongs to a Bundle manifest.

**1.6-60 — ACCEPTED**

A Bundle can be used from different UserSessions without changing the immutable Bundle itself.

**1.6-61 — ACCEPTED**

`luna-bundle` does not own physical installation paths such as `DATA/system/apps`. Installation/storage ownership remains outside the Bundle domain model.

## 62–70 — Application/update planning

**1.6-62 — ACCEPTED**

`luna-app-manager` constructs/validates an `ApplicationPlan` without launching the application.

**1.6-63 — ACCEPTED**

An `ApplicationPlan` must be prevalidated for dependencies, compatibility, security authorization, resource requirements and migration requirements before execution.

**1.6-64 — ACCEPTED**

An invalid plan cannot enter the mutation stage.

**1.6-65 — ACCEPTED**

`UpdatePlan` belongs to `luna-update-manager`. It executes accepted plans but does not redefine manager-owned domain semantics.

**1.6-66 — ACCEPTED**

Update operations use the conceptual transaction sequence:

```text
prepare
checkpoint
apply
verify
commit
```

**1.6-67 — ACCEPTED**

Before commit, old state remains authoritative wherever the specific operation can provide that guarantee.

**1.6-68 — ACCEPTED**

If an operation terminates after mutation has started, reconciliation determines whether the operation committed, partially committed or did not commit. Failure does not imply automatic rollback.

**1.6-69 — ACCEPTED**

Rollback is an explicit recovery/transaction action rather than an automatic response to every crash.

**1.6-70 — ACCEPTED**

`luna-state` may store an operation/checkpoint reference but does not own or encode snapshot internals.

## 71–75 — Events and persistent state backend

**1.6-71 — ACCEPTED**

Events support correlation metadata sufficient to associate them with an operation and its source/target context.

**1.6-72 — ACCEPTED**

Live event subscription and persistent event history are separate concerns.

**1.6-73 — ACCEPTED**

Event subscriptions have an explicit lifecycle such as subscribe/active/cancelled/completed.

**1.6-74 — ACCEPTED**

Closing the CLI/GUI does not cancel a backend operation merely because the client disconnected, and it does not erase queryable operation state.

**1.6-75 — ACCEPTED — option C**

`luna-state` contains the state domain model plus storage traits/backend implementations needed to persist that state, while checkpoint/snapshot mechanisms remain a separate concern. The state domain must not become Btrfs-specific.

## Canonical interpretation

These decisions establish the following boundaries:

```text
luna-fs
    = low-level filesystem primitives

luna-root-mapping
    = logical/physical composition and mapping plans

luna-security
    = policy, grants and trust

luna-bundle
    = immutable Bundle domain

luna-app-manager
    = application planning/management

luna-update-manager
    = mutation/transaction execution

luna-state
    = persistent logical state + storage abstraction

checkpoint mechanism
    = separate snapshot/rollback mechanism

luna-event
    = correlated event delivery + history
```
