# Project Luna — Manager and Runtime API Contracts (Phase 1.6)

**Status:** Design/implementation baseline
**Source of truth:** `docs/ARCHITECTURE.md`
**Related:** `docs/development/API-CONTRACTS-1.6.md`, `docs/architecture/CRATE-MAP.md`

This document records the first public API boundaries for the management and runtime layer. It deliberately keeps OS-specific implementation details out of the domain contracts.

## 1. `luna-system-manager`

Owns the System Image state model and query side only. It does not perform update mutations.

Implemented concepts:

- `SystemImageRef` — versioned logical reference to a System Image;
- `SystemState` — current and factory image references;
- `SystemQuery` — read/query boundary.

The model deliberately keeps `current` and `factory` separate. The factory image is the original installed system image and is not an ordinary retention candidate.

## 2. `luna-kernel-manager`

Owns kernel inventory/selection queries. It does not perform installation, update, or removal mutations.

Implemented concepts:

- `KernelRef`;
- `KernelSelection` — current kernel, previous kernel, and immutable factory kernel;
- `KernelQuery`.

The compatibility algorithm remains to be connected to System Image manifests and the boot specification. The factory kernel is explicitly modeled because Factory is an image+kernel recovery point.

## 3. `luna-update-manager`

Owns execution of change requests.

Implemented concepts:

- `UpdateOperation`;
- `UpdatePlan`;
- `UpdateError`;
- `UpdateExecutor`.

The plan represents requested changes. The executor performs mutations. Desired state remains owned by the relevant system/application/kernel model.

## 4. `luna-app-manager`

Owns application lifecycle management, not application execution.

Implemented concepts:

- `ApplicationRef`;
- `ApplicationOperation`;
- `AppManagerError`;
- `ApplicationManager`.

Supported operation categories in the current baseline:

- install;
- update;
- removal;
- verification;
- migration.

Package-import (`.deb` / `.rpm`), bundle assembly, dependency extraction, and manifest analysis remain bounded by the later bundle specification.

## 5. `luna-device-manager`

Owns device and volume discovery/model queries.

Implemented concepts:

- `DeviceId`;
- `VolumeId`;
- `VolumeState`;
- `VolumeInfo`;
- `DeviceQuery`.

Mount authorization, application permissions, and security policy remain outside this crate.

## 6. `luna-system-runtime`

Owns system-wide runtime supervision. There is one system runtime for the operating system.

Implemented concepts:

- `RuntimeState`;
- `SystemRuntime`;
- a stable session-state event type;
- explicit consumption of `luna-event` and `luna-user-session` boundaries.

The runtime supervises multiple UserSessions and is not duplicated per user.

## 7. `luna-user-session`

A `UserSession` is one domain instance combining user identity and interactive session state.

Implemented concepts:

- `SessionId`;
- `SessionState`;
- `UserSession`;
- validated lifecycle transitions.

The model deliberately does not expose Linux TTY sessions as the Luna abstraction.

## 8. `luna-app-runtime`

Owns application execution and `ApplicationInstance` lifecycle.

Implemented concepts:

- `ApplicationInstanceId`;
- `InstanceState`;
- `ApplicationInstance`;
- `ApplicationRuntime`.

The runtime receives a bundle manifest and namespace mapping and can consult the Security authority. It does not install, update, or remove applications.

## 9. `luna-cli`

Remains a thin client. Final command tree, alias grammar, and IPC transport are intentionally not frozen.

The architecture permits user-friendly aliases such as `app i`, `sys u`, and `dev list`, while routing to the same backend services that a GUI will use.

## 10. Integration contract

The current API boundaries are intended to compose in this direction:

```text
BundleManifest
      │
      ▼
MappingTable ───────► ApplicationRuntime
      │                       │
      ▼                       ▼
   luna-fs              Security policy
                              │
                              ▼
                         Decision

UserSession ──────────► ApplicationInstance
      ▲                       │
      │                       ▼
SystemRuntime ─────── supervision

SystemManager ───────► desired/query state
KernelManager ───────► kernel state
AppManager ──────────► application lifecycle requests
DeviceManager ───────► device/volume state
UpdateManager ───────► mutation execution
```

No line in this diagram implies that the crates must all live in one process. Process separation and IPC remain later implementation decisions.

## 11. Current API status

The following contracts are now explicit enough for integration-test design:

- shared identity/version values;
- low-level filesystem operations;
- per-namespace exact-file mapping;
- layered configuration lookup;
- security authorization requests;
- durable generic state;
- event publication/subscription;
- bundle domain metadata/resources;
- user-session lifecycle;
- system image query state;
- kernel current/previous/factory state;
- update plans;
- application lifecycle operations;
- device/volume query state;
- system runtime supervision;
- application instance lifecycle.

These are **API/domain baselines**, not declarations that the operating system is implemented.

## 12. Explicitly deferred

- actual persistence backends;
- update transaction protocol;
- System Image manifest consumption;
- kernel compatibility resolver implementation;
- device automount backend;
- namespace materialization;
- permission enforcement implementation;
- concrete IPC;
- final CLI grammar and aliases;
- GUI implementation;
- `.deb` / `.rpm` conversion internals;
- Bundle Format v1 wire/archive representation.
