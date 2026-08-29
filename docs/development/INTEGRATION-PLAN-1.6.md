# Project Luna — Integration Plan (Phase 1.6)

**Status:** Active — backend implementation transition
**Source of truth:** `docs/ARCHITECTURE.md`

This document defines the cross-crate verification and implementation sequence after the foundation, manager/runtime API passes and the first integration prototypes.

## 1. Mapping + security

Verify that a namespace can represent logical mappings while `luna-security` independently decides whether the caller may use the resulting resource.

```text
logical requirement
    ↓
luna-root-mapping
    ↓
validated MappingPlan
    ↓
luna-security
    ↓
allow / deny / ask / constrained
    ↓
Linux enforcement primitives
```

The mapping layer must never become the permission authority.

## 2. Configuration precedence

Verify semantic-class-specific layering. For ordinary application configuration the current model is:

```text
user override
    ↓
application/default content
    ↓
DATA/system/config
    ↓
System Image default
```

Removing a user override exposes the lower layer again. System-wide settings remain independent of the active `UserSession`.

## 3. Bundle + mapping

Verify that immutable Bundle resources describe logical paths without exposing physical `DATA/system/apps/...` paths to applications.

File mappings are the default. Explicit subtree mappings are allowed for suitable semantic resources such as shared library trees.

Conflicting mappings within one namespace are errors; identical immutable definitions may be deduplicated where safe.

## 4. Session + application instance

Verify the accepted hierarchy:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

One system runtime supervises multiple `UserSession`s. `UserSession` combines user/session identity. `ApplicationInstance` owns runtime-specific process/lifecycle state.

## 5. Manager state + update plans

Verify that:

- `luna-system-manager` owns system state/query semantics;
- `luna-kernel-manager` owns kernel inventory/metadata/compatibility queries;
- `luna-app-manager` owns application management plans and lifecycle operations;
- `luna-update-manager` executes state-changing mutations;
- GUI/CLI are not owners of backend operations.

## 6. Event delivery boundaries

Verify that:

- EventId and OperationId remain independent;
- ordering is monotonic per operation;
- timestamps are metadata only;
- Ephemeral/Persistent/Audit retention classes stay distinct;
- persistent events are replayable;
- bounded delivery/backpressure prevents unbounded memory growth;
- Audit events are never silently dropped.

The implementation must remain lightweight; Kafka is conceptual only.

## 7. Real Linux namespace + materialization prototype

This is now the first high-risk backend target.

Goals:

- create the actual Linux mount namespace;
- construct the normal Linux-compatible logical `/`;
- apply validated per-namespace mappings;
- support controlled file and approved subtree mappings;
- use bind mounts and related existing Linux primitives where suitable;
- keep policy in `luna-security`;
- prevent traversal/symlink escape across authorized physical boundaries;
- avoid exposing a fake container filesystem identity to applications.

Application namespaces may be implemented with Linux namespaces without making the application aware that it is in a container-like environment.

## 8. Persistent state + transaction prototype

Implement a durable backend behind the synchronous `luna-state` abstraction.

Required semantics:

- atomic multi-mutation transactions;
- global revision conflict detection;
- no partial writes on a stale revision;
- crash-safe reopen/recovery;
- persistent state under `DATA/system/state/`;
- no custom second WAL when the underlying backend already provides the required durability.

The current implementation direction is a small embedded transactional key/value backend; the backend choice remains an implementation detail rather than a new architectural layer.

## 9. Update/checkpoint/rollback engine

Implement the real `luna-update-manager` mutation path:

```text
prepare
   ↓
checkpoint
   ↓
apply
   ↓
verify
   ↓
commit
```

On interruption, reconcile the durable operation state. Rollback is explicit and checkpoint-backed where supported; it is not an automatic response to every application crash.

## 10. RFC-0002 — Bundle Format v1

Design and accept the final Bundle Format v1 before treating the `.lbp` representation as stable.

Requirements already fixed by architecture:

- `.lbp` is a Bundle transport/archive representation;
- installed Bundle remains immutable;
- Bundle identity is BundleId + Version + ContentIdentity;
- manifest describes identity/resources/dependencies/capabilities/entry points;
- runtime state is not stored as Bundle manifest state;
- `luna-bundle` owns Bundle domain/format concerns;
- `luna-app-manager` owns install/update/remove/import lifecycle.

## 11. Security/signature chain

Implement the production separation between:

```text
signature verification
        ↓
trust decision
        ↓
permission policy
        ↓
runtime enforcement
```

Publisher identity, repository/distribution metadata, content identity and local trust must remain distinguishable.

## 12. Boot/system-state integration

The bootloader remains a separate implementation boundary. The userspace side must eventually provide:

- boot success confirmation;
- persistent boot-state metadata;
- image/kernel compatibility resolution;
- recovery/factory selection support.

`luna-boot.efi` itself is not part of this userspace workspace.

## 13. Resource enforcement

Integrate Linux resource-control mechanisms for:

- CPU;
- memory;
- process count;
- file descriptors;
- I/O and storage constraints where needed;
- protected system-critical resource budget.

The policy is Luna-specific; cgroups and related kernel mechanisms are enforcement primitives.

## 14. Device/volume integration

Implement device lifecycle and automount so external volumes can appear as:

```text
DATA/system/volumes/<friendly-name>
```

and in the file manager's Volumes view without manual mount commands. Auto-execution from removable media remains policy-controlled and must not happen implicitly.

## 15. End-to-end verification

After the real backends exist, validate the complete chain:

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux kernel
 ↓
logical Linux root
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
luna-app-runtime
 ↓
ApplicationInstance
```
