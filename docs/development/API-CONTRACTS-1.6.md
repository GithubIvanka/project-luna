# Project Luna — API Contracts (Phase 1.6)

**Status:** Design baseline
**Source of truth:** `docs/ARCHITECTURE.md`
**Related:** `docs/development/CRATE-MAP-1.6.md`

This document defines the first public API boundaries. It intentionally specifies concepts and ownership before implementation details. Exact Linux mechanisms, IPC, serialization formats, and concrete dependencies remain deferred unless explicitly stated here.

## 1. Design rules

1. Public APIs expose domain contracts, not implementation details.
2. Foundational crates must not acquire higher-level policy merely to make another crate convenient.
3. Ownership is singular: one subsystem owns a piece of state or policy; other crates query or invoke it through contracts.
4. Filesystem primitives are not permission policy.
5. Logical-root mapping is not authorization.
6. Configuration is layered data, not runtime state management.
7. Async execution is expected at higher layers, but `luna-common` remains synchronous and dependency-light.
8. Error types are local to the owning crate unless an error is genuinely a foundational value contract.
9. Serialization is not added to a foundational type merely for convenience.
10. Public APIs should remain usable by both CLI and GUI backends without UI-specific concepts.

## 2. `luna-common`

### Responsibility

Minimal shared value types and contracts that are genuinely common to multiple crates.

### Current public types

- `BundleId`
- `ComponentId`
- `Version`

The existing implementation is intentionally opaque: callers can construct and inspect values, but higher-level validity rules remain with the owning subsystem.

### Contract

```rust
pub struct BundleId(/* opaque */);
pub struct ComponentId(/* opaque */);
pub struct Version {
    /* semantic version value */
}
```

Required characteristics:

- deterministic equality and ordering;
- hashing where appropriate;
- `Display` for human-readable identifiers/versions;
- no filesystem access;
- no configuration loading;
- no async runtime dependency;
- no security policy;
- no manager/runtime service interfaces.

### Deliberately not included yet

- generic `LunaError`;
- UUID policy;
- path types;
- permission/capability types;
- IPC request/response types;
- bundle manifest types;
- configuration types;
- serialization traits/formats.

Those concepts have owners elsewhere and should not be moved into `luna-common` prematurely.

## 3. `luna-fs`

### Responsibility

Low-level filesystem abstraction over the underlying Linux filesystem mechanisms.

### Boundary

`luna-fs` answers: **how do we perform a filesystem operation?**

It does not answer: **is this operation allowed?**, **what logical path should an application see?**, or **where should an application be installed?**

### Initial conceptual API

```rust
pub trait FileSystem {
    type Error;

    fn open(&self, path: &Path) -> Result<FileHandle, Self::Error>;
    fn create(&self, path: &Path) -> Result<FileHandle, Self::Error>;
    fn remove(&self, path: &Path) -> Result<(), Self::Error>;
    fn metadata(&self, path: &Path) -> Result<FileMetadata, Self::Error>;
}
```

The exact trait shape is provisional. The important contract is the separation of low-level filesystem operations from policy and logical mapping.

`luna-fs` may expose handles, metadata, directory operations, mount-related primitives, and other filesystem mechanisms as they become necessary. It must not grow a user-facing policy layer.

## 4. `luna-root-mapping`

### Responsibility

Describe and resolve mappings between physical resources and the logical filesystem presented to a process.

### Boundary

`luna-root-mapping` answers: **what physical resource is represented by this logical path?**

It does not answer whether the caller is authorized to access that resource.

### Initial conceptual API

```rust
pub struct LogicalPath(/* opaque logical path */);
pub struct PhysicalPath(/* opaque physical path */);
pub struct MappingTable(/* mapping set */);

pub trait RootMapping {
    type Error;

    fn resolve(&self, path: &LogicalPath) -> Result<PhysicalPath, Self::Error>;
}
```

The mapping model must support the Luna requirement that an application can see paths such as `/bin/app` while the actual application resource may live inside an installed bundle, for example under `DATA/system/apps/...`.

The mapping layer may also represent user/system data overlays and removable volumes. Authorization remains outside this crate.

## 5. `luna-config`

### Responsibility

Typed configuration representation, layered configuration lookup, and persistence contracts.

### Configuration scopes

The accepted model has at least these conceptual layers:

1. user configuration;
2. application configuration within the user context;
3. system-wide configuration under `DATA/system/config`.

Application settings follow the overlay model discussed in Phase 1.5: user data overrides application defaults; if no user override exists, application-provided defaults are used; system defaults remain the lower fallback where applicable.

### Initial conceptual API

```rust
pub enum ConfigScope {
    System,
    User(UserId),
    Application { user: UserId, application: BundleId },
}

pub trait ConfigStore {
    type Error;

    fn get(&self, scope: ConfigScope, key: &str) -> Result<Option<ConfigValue>, Self::Error>;
    fn set(&self, scope: ConfigScope, key: &str, value: ConfigValue) -> Result<(), Self::Error>;
    fn remove(&self, scope: ConfigScope, key: &str) -> Result<(), Self::Error>;
}
```

`UserId` and `ConfigValue` are intentionally conceptual here. Their final ownership and representation must be decided before implementation. They must not be added to `luna-common` simply because this draft uses them.

### Important boundary

`luna-config` stores and resolves configuration. It does not decide whether a user or application is permitted to change a configuration item. That policy belongs to `luna-security`.

## 6. Dependency direction for the first wave

The desired direction is:

```text
luna-common
    ▲
    │
luna-fs
    ▲
    │
luna-root-mapping

luna-common
    ▲
    │
luna-config
```

This is a conceptual dependency direction, not a requirement that every crate directly depend on every lower crate.

In particular:

- `luna-common` must remain independent;
- `luna-root-mapping` may consume filesystem abstractions but must not absorb filesystem policy;
- `luna-config` must not depend on runtime/manager crates;
- higher-level crates may consume these contracts later.

## 7. Implementation gate

Before creating each crate, its public contract must be stable enough to answer:

- what it owns;
- what it does not own;
- what its public inputs and outputs are;
- what errors belong to it;
- which lower-level crates it may depend on;
- which concepts are deliberately deferred.

The next implementation target is `luna-common`, followed by `luna-fs`, `luna-root-mapping`, and `luna-config`.
