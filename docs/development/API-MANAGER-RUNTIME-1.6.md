# Project Luna — Manager and Runtime API Contracts (Phase 1.6)

**Status:** Design/implementation baseline
**Source of truth:** `docs/ARCHITECTURE.md`
**Related:** `docs/development/API-CONTRACTS-1.6.md`, `docs/architecture/CRATE-MAP.md`

This document records the first public API boundaries for the management and runtime layer. It deliberately keeps implementation details out of the domain contracts.

## 1. `luna-system-manager`

Owns the system image state model and query side only. It does not perform update mutations.

Implemented concepts:

- `SystemImageRef` — versioned logical reference to a System Image;
- `SystemState` — current and factory image references;
- `SystemQuery` — read/query boundary.

The model deliberately keeps `current` and `factory` separate.

## 2. `luna-kernel-manager`

Owns kernel inventory/selection queries. It does not perform installation, update, or removal mutations.

Implemented concepts:

- `KernelRef`;
- `KernelSelection` — current kernel plus the previous kernel available for fallback;
- `KernelQuery`.

The compatibility algorithm itself remains to be connected to System Image manifests and the boot specification.

## 3. `luna-update-manager`

Owns execution of change requests.

Implemented concepts:

- `UpdateOperation`;
- `UpdatePlan`;
- `UpdateError`;
- `UpdateExecutor`.

The plan represents requested changes. The executor performs them. Desired system/kernel/application state remains owned by the relevant manager/model.

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

The final package-import model for `.deb` / `.rpm`, bundle assembly, dependency extraction, and migration semantics remains to be expanded after the bundle contract is mature.

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
- a stable event type for session state changes;
- explicit consumption of the `luna-event` and `luna-user-session` boundaries.

The runtime supervises multiple UserSessions. It is not duplicated per user.

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

The runtime receives a bundle manifest and namespace mapping, then operates under the security authority. It does not install/update/remove applications.

## 9. `luna-cli`

Remains a thin client. Final command tree and alias grammar are intentionally not frozen yet.

The architecture permits user-friendly aliases such as `app i`, `sys u`, and `dev list`, but the exact parser/service transport is a later contract.

## 10. Dependency direction

```text
                clients
           ┌──────┴──────┐
           ▼             ▼
      luna-cli        future GUI
           │             │
           └──────┬──────┘
                  ▼
          management APIs
                  │
      ┌───────────┼───────────┐
      ▼           ▼           ▼
 system-manager kernel-manager device-manager
      │           │           │
      └───────┬───┴──────┬────┘
              ▼          ▼
       update-manager   security
              │
              ▼
         runtime layer
        ┌─────┴─────┐
        ▼           ▼
 system-runtime  UserSession
                    │
                    ▼
               app-runtime
                    │
             root-mapping
                    │
                   fs
```

The diagram is conceptual. Exact IPC and process separation remain implementation decisions.

## 11. Explicitly deferred

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
