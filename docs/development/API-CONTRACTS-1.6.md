# Project Luna — API Contracts (Phase 1.6)

**Status:** Foundation/domain baseline implemented
**Source of truth:** `docs/ARCHITECTURE.md`
**Related:** `docs/development/CRATE-MAP-1.6.md`

This document records the current API boundary for the first implementation wave. It defines ownership and stable concepts before OS-specific implementation. Exact Linux mechanisms, IPC transport, serialization formats, and persistence backends remain deferred unless explicitly stated.

## 1. Global rules

1. Public APIs expose domain contracts, not implementation accidents.
2. Ownership is singular: one subsystem owns a piece of state or policy.
3. `luna-common` contains only genuinely shared value types.
4. Filesystem primitives are not authorization policy.
5. Logical-root mapping is not authorization.
6. Configuration lookup is not permission evaluation.
7. Error types belong to their owning crate unless they are genuinely foundational.
8. Lower layers must not acquire higher-layer dependencies just for convenience.
9. GUI and CLI are clients of the same backend contracts.
10. Linux mechanisms are implementation primitives; they do not redefine Luna's domain model.

## 2. `luna-common`

### Responsibility
Minimal shared identity/version value types.

### Implemented public types

- `BundleId`
- `ComponentId`
- `UserId`
- `Version`

The identifier types are opaque wrappers around their textual representation. Canonical syntax validation remains with the owning subsystem. `Version` is a small semantic version value with deterministic ordering and formatting.

### Must not contain

- generic system-wide error enums;
- filesystem operations;
- security policy;
- runtime state;
- configuration storage;
- IPC contracts;
- serialization policy.

## 3. `luna-fs`

### Responsibility
Low-level filesystem primitives.

### Implemented boundary

```rust
pub trait FileSystem {
    fn open(&self, path: &Path) -> Result<FileHandle, FsError>;
    fn create(&self, path: &Path) -> Result<FileHandle, FsError>;
    fn remove(&self, path: &Path) -> Result<(), FsError>;
    fn metadata(&self, path: &Path) -> Result<FileMetadata, FsError>;
}
```

`HostFileSystem` is the initial host-backed implementation used for tests and early development.

### Boundary exclusions

`luna-fs` does not decide:

- whether a caller may access a path;
- which physical path corresponds to a logical path;
- which files belong to an application;
- configuration precedence;
- bundle lifecycle.

## 4. `luna-root-mapping`

### Responsibility
Per-namespace logical filesystem mapping.

### Implemented concepts

- `LogicalPath`
- `PhysicalPath`
- `MappingRule`
- `MappingTable`
- `MappingError`

A `MappingTable` belongs to one logical namespace. A rule maps an **individual logical file** to one backing path. Directory-wide implicit mapping is deliberately not part of this first contract.

Example concept:

```text
/bin/app
    ↓
DATA/system/apps/example/resources/bin/app
```

Each application namespace may have its own mapping table. Authorization is not evaluated here; `luna-security` owns policy.

## 5. `luna-config`

### Responsibility
Configuration representation, scoped storage, and layered lookup.

### Implemented concepts

- `ConfigScope`
- `ConfigKey`
- `ConfigValue`
- `ConfigStore`
- `MemoryConfigStore`
- `LayeredConfig`

### Accepted precedence for application settings

```text
User/Application override
        ↓
Application default
        ↓
System default
```

System-wide mutable settings conceptually live under `DATA/system/config`; user/application overrides belong to the relevant user-owned configuration space. The concrete TOML file layout and serialization API are deferred.

`luna-config` does not authorize changes. Security owns authorization.

## 6. `luna-security`

### Responsibility
Central policy authority.

### Implemented concepts

- `Principal`
- `Resource`
- `Permission`
- `AuthorizationRequest`
- `Decision`
- `PolicyAuthority`

The policy layer can evaluate requests involving users, applications, system resources, user data, application data, devices, and volumes.

Low-level kernel/filesystem mechanisms still enforce the resulting decision.

No global `root`/`sudo` abstraction is introduced by this crate.

## 7. `luna-state`

### Responsibility
Durable state boundary.

### Implemented concepts

- `StateKey`
- `StateValue`
- `StateError`
- `StateStore`

This crate is intentionally generic. Specialized state remains with its owning subsystem:

- boot state → boot subsystem;
- configuration → `luna-config`;
- security policy → `luna-security`;
- system model → `luna-system-manager`.

## 8. `luna-event`

### Responsibility
Event-domain contracts.

### Implemented concepts

- `EventType`
- `Event`
- `EventPublisher`
- `EventSubscriber`
- `Subscription`

The crate defines the message contract only. It is not a Kafka clone and does not select the final broker/channel implementation. Tokio remains an accepted higher-level async-runtime direction.

## 9. `luna-bundle`

### Responsibility
Internal bundle domain model.

### Implemented concepts

- `BundleKind`
- `BundleMetadata`
- `BundleResource`
- `BundleManifest`
- `BundleError`
- `validate_manifest`

The domain model is separate from `.lbp`. RFC-0002 remains the authority for the future transport/archive representation and is **not accepted as final yet**.

The current resource model explicitly supports the Luna principle that a logical executable such as `/bin/app` can originate from a resource stored deep inside the application bundle.

## 10. `luna-user-session`

### Responsibility
One user's interactive session as a single domain instance.

### Implemented concepts

- `SessionId`
- `SessionState`
- `UserSession`
- lifecycle transition validation

The architectural hierarchy remains:

```text
system-runtime
├── UserSession A
│   └── app-runtime
└── UserSession B
    └── app-runtime
```

This is a Luna domain model, not a Linux TTY abstraction.

## 11. Higher-level scaffolds

The following crates remain intentionally at scaffold/ownership-boundary level until their dependent contracts are sufficiently mature:

- `luna-app-manager`
- `luna-system-manager`
- `luna-update-manager`
- `luna-device-manager`
- `luna-kernel-manager`
- `luna-system-runtime`
- `luna-app-runtime`
- `luna-cli`

Their responsibilities are fixed by `CRATE-MAP.md`, but detailed public APIs should be derived from the already-established lower-level contracts instead of being guessed independently.

## 12. Dependency direction

The first stable direction is:

```text
                 luna-common
                /     |     \
               /      |      \
          luna-fs  luna-config  luna-security
             |
      luna-root-mapping

luna-common → luna-state
luna-common → luna-event
luna-common → luna-bundle
luna-common → luna-user-session
```

Higher-level managers and runtimes consume these contracts. Lower-level crates do not depend upward on managers or runtimes.

## 13. Implementation gate

A crate may move from scaffold to implementation when its API can answer:

- what it owns;
- what it does not own;
- what its public inputs and outputs are;
- which errors belong to it;
- which lower-level crates it may depend on;
- which persistence and transport details are still deferred.

## 14. Current implementation status

Completed in this API pass:

- `luna-common` value-type foundation refined;
- `luna-fs` low-level contract implemented;
- `luna-root-mapping` exact-file mapping contract implemented;
- `luna-config` layered configuration contract implemented;
- `luna-security` policy contract implemented;
- `luna-state` persistence boundary implemented;
- `luna-event` event boundary implemented;
- `luna-bundle` domain model implemented without final `.lbp` wire format;
- `luna-user-session` lifecycle model implemented.

The next work is to derive the manager/runtime APIs from these contracts and then add integration tests across the boundaries.
