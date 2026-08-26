# Project Luna — Phase 1.1 — Logical Root, Path Mapping & Namespace Model

**Status:** Accepted working architecture
**Date:** 2026-08-16

## Scope

Phase 1.1 defines how Luna can present a conventional Linux filesystem to the kernel, drivers and applications while physically keeping the user-visible storage clean and organized.

## 1. Fundamental problem

Linux software expects conventional paths such as:

```text
/etc
/usr
/home
/var
/lib
/bin
```

Luna intentionally does not want these directories physically spread across the user's DATA partition. Instead, the runtime constructs a logical Linux-compatible root in RAM/virtual filesystem space and maps selected paths to Luna's physical storage.

The physical DATA structure remains clean and Luna-native.

## 2. Init/root architecture

The accepted direction is a hybrid of existing Linux mechanisms rather than inventing an entire init/mount system from scratch, while also avoiding putting all responsibilities into one monolithic init component.

The runtime responsibility is centered around `luna-root`.

`luna-root` should establish the logical root environment and coordinate the controlled path composition needed by the rest of the system.

The exact process split between `luna-root`, init/service management and filesystem helpers remains an implementation question.

## 3. Minimal logical root

The system first establishes a minimal RAM-based logical root.

Conceptually:

```text
Linux kernel
    ↓
minimal logical root in RAM
    ↓
System Image logical content
    ↓
DATA-backed mappings
    ↓
application/user namespaces
```

The root is logically `/`, but the physical storage is not a normal Linux root filesystem sitting on a disk mount.

## 4. Hybrid System Image loading

The System Image remains SquashFS and is accessed lazily/hybridly:
- only data required for early operation should be made available immediately;
- other SquashFS blocks should be obtainable on demand;
- the logical root remains the application's/kernel's normal `/` view;
- the exact kernel/SquashFS/RAM mechanism is not yet finalized.

The objective is to avoid loading a large System Image into RAM unnecessarily while still making the OS appear as a normal Linux filesystem.

## 5. Path mapping is controlled composition, not arbitrary rewriting

Luna must not implement unrestricted path substitution.

Mappings require explicit rules defining:
- which physical Luna locations may be mapped;
- which logical Linux path classes they may satisfy;
- which namespace may receive the mapping;
- whether the mapping is read-only or writable.

Example of an allowed conceptual mapping:

```text
logical /etc
    ↓
user-specific configuration in DATA/users/<user>/config
    ↓ fallback
System Image default configuration
```

An arbitrary path such as:

```text
DATA/users/<user>/config/bin
```

must not automatically become a logical executable `/bin` simply because the directory exists.

This prevents the mapping layer from becoming an unrestricted filesystem overlay engine.

## 6. Per-namespace mapping tables

There is **no single global mapping table**.

Every application namespace receives a small table containing only mappings required by that namespace.

Example:

```text
Application A namespace
/lib/gtk → DATA/system/libs/gtk/3

Application B namespace
/lib/gtk → DATA/system/libs/gtk/4
```

Both applications can see their own logical `/lib/gtk` while the physical library versions remain isolated.

This is a deliberate dependency-isolation mechanism inspired by the useful part of Nix-like layouts without turning Luna into NixOS.

## 7. Layered lookup

The accepted conceptual resolution order is:

```text
application layer
       ↓
user layer
       ↓
system layer
```

This resembles Python's layered lookup idea conceptually, but Luna's implementation is filesystem-namespace policy, not Python import semantics.

A mapping lookup should first consider resources explicitly provided to the application, then user-specific state, then immutable/system defaults.

## 8. Application namespace

Every application gets its own mount/filesystem namespace.

Conceptually:

```text
Application
    ↓
application namespace
    ├── application bundle
    ├── required libraries
    ├── user data/config mappings
    ├── required system paths
    └── explicitly permitted external volumes
```

The application must not automatically see the entire host filesystem.

## 9. Dependency isolation

If two applications require incompatible dependency versions, each namespace receives a different mapping:

```text
App A → GTK 3
App B → GTK 4
```

The application itself continues to see the expected conventional Linux path.

This allows old and new software to coexist without forcing a single global library version.

## 10. `/etc`, `/usr`, `/home`, `/var` and similar paths

These paths should be treated as **logical compatibility interfaces**, not as a requirement to physically recreate the traditional Linux directory hierarchy on DATA.

### `/etc`
Primarily configuration. User-modified configuration should resolve to `DATA/users/<user>/config`, while unchanged defaults come from the System Image.

### `/home`
Maps to the active user's `DATA/users/<user>/home`.

### Application data
Application-specific mutable state maps to the active user's `DATA/users/<user>/data` rather than being written into the application bundle.

### `/var`
Should be decomposed according to actual semantic purpose rather than copied wholesale. Mutable state that belongs to the system can map to an appropriate Luna-managed DATA location; caches remain separately managed. Exact `/var` mapping classes remain to be specified.

### `/usr`, `/bin`, `/lib` and related system paths
These primarily represent immutable System Image content plus namespace-local dependency mappings. They should not become writable user paths by default.

The exact list of mappings for each conventional Linux path remains a design task. The important accepted rule is that mappings are semantic and policy-controlled, not a blanket mirror of DATA into `/`.

## 11. Why namespaces and mappings are separate concepts

Mount namespaces provide isolation.

Mapping policy decides what an isolated namespace contains.

Therefore:

```text
namespace = boundary
mapping table = composition rules inside the boundary
permission policy = authorization for what may enter the boundary
```

Keeping these concepts separate should make the architecture easier to reason about and reduce cross-application conflicts.

## 12. Persistent namespace state

A namespace should not necessarily be reconstructed from zero on every application restart.

The runtime may keep a small amount of state associated with the application namespace, including its mapping table and runtime metadata. The state can remain in RAM for a managed period after application exit; roughly one hour was discussed as an example, but the timeout is not final.

## 13. Application lifecycle relationship

Phase 1.1 provides the namespace/mapping foundation used by Phase 1.2's application lifecycle and session model.

A future application lifecycle must be able to:
- create a namespace;
- compose its mappings;
- launch the bundle;
- preserve relevant namespace state;
- suspend/restrict or terminate it according to user/session policy;
- recreate it consistently without losing its intended mapping configuration.

## 14. Explicitly not finalized

- exact `luna-root` process architecture;
- exact Linux mount API sequence;
- exact SquashFS lazy-loading implementation;
- exact mapping-table format;
- exact path-class taxonomy;
- exact read/write policy for every path class;
- exact `/var` decomposition;
- exact namespace persistence mechanism;
- exact integration with service manager.
