# Project Luna — Crate Map (Phase 1.6)

**Status:** Design baseline
**Source of truth:** `docs/ARCHITECTURE.md`
**Purpose:** translate the accepted architecture into Rust crate boundaries before API design and implementation.

> This document is a crate-boundary map, not an implementation commitment. A crate is created only when its boundary and public responsibility are sufficiently defined.

## 1. Rules

1. `luna-common` remains deliberately small and contains only foundational value types and contracts that are genuinely shared.
2. Higher-level managers must not be placed into `luna-common`.
3. Runtime, manager, security, filesystem, configuration, and boot responsibilities remain separate.
4. A CLI is a client of backend services; it does not own backend logic.
5. GUI and CLI must be able to address the same backend through the same service/API contracts.
6. `luna-app-manager` owns application installation, update, removal, verification and migration orchestration; application launching belongs to the runtime/system chain.
7. `luna-update-manager` executes system/kernel/application update operations according to the owning model's rules.
8. `luna-system-manager` owns system-state modeling/query responsibilities.
9. `luna-kernel-manager` owns kernel model/query responsibilities; installation/update/removal work is executed through the update path.
10. `luna-security` is the central policy authority. Low-level filesystem/kernel mechanisms enforce the resulting restrictions.
11. `luna-system-runtime` supervises runtime activity and resource isolation; `luna-app-runtime` owns application execution.
12. User identity and session state are represented by a `UserSession` concept rather than a Linux-style collection of independent TTY sessions.
13. `luna-root-mapping` owns logical-root/path mapping concepts; applications see their logical filesystem rather than the physical bundle layout.
14. `.lbp` is a transport/archive format for bundles, not the bundle's internal runtime model.
15. Bundle internals and manifest semantics belong to `luna-bundle`; lifecycle policy belongs to managers/runtime as appropriate.

## 2. Foundational crates

### `luna-common`

**Role:** minimal shared foundation.

Expected contents:
- stable identifier/value types;
- small version/value primitives;
- foundational shared contracts only when they have no higher-level owner.

Must not contain:
- filesystem policy;
- security policy;
- application management;
- runtime management;
- logging subsystem;
- configuration subsystem;
- OS-specific orchestration.

### `luna-fs`

**Role:** low-level filesystem abstraction.

Owns:
- filesystem primitives and low-level filesystem-facing abstractions;
- filesystem operations needed by higher layers;
- abstraction over the underlying Linux filesystem mechanisms.

Does not own:
- application policy;
- permission policy;
- bundle lifecycle;
- user-facing path aliases.

### `luna-root-mapping`

**Role:** logical filesystem/root mapping.

Owns:
- mapping physical resources into the logical root visible to a process;
- path-resolution/mapping models;
- bundle-to-logical-path mappings;
- mechanisms needed for `/bin`, `/usr/bin`, data overlays and similar logical views.

It does not decide whether an application is allowed to access a resource; that belongs to Security.

### `luna-config`

**Role:** configuration model and access layer.

Owns:
- typed configuration representation;
- system-wide configuration;
- user-scoped configuration;
- application configuration overlays;
- configuration precedence and persistence contracts.

Configuration remains separate from runtime and managers.

## 3. Bundle and application management

### `luna-bundle`

**Role:** bundle internal model.

Owns:
- bundle structure;
- bundle manifest model;
- bundle resources/components;
- bundle identity/version metadata;
- internal bundle validation primitives;
- future Bundle Format v1 model.

`.lbp` is treated as an archive/transport representation of a bundle, not as the bundle runtime itself.

### `luna-app-manager`

**Role:** application lifecycle management backend.

Owns:
- install;
- update;
- removal;
- verification/integrity checks;
- compatibility checks;
- migrations associated with application updates;
- importing supported external packages such as `.deb` and `.rpm` and converting them into Luna bundles;
- constructing logical mappings required by installed applications.

Does not own application execution.

### `luna-update-manager`

**Role:** execution engine for updates.

Owns:
- applying requested update operations;
- acquiring changed/new artifacts;
- assembling new system/application/kernel states;
- update transactions and checkpoints;
- executing the mutation side of the update model.

The owning manager/model remains responsible for what state is desired.

## 4. System, kernel and device management

### `luna-system-manager`

**Role:** system-state owner/model and query backend.

Owns:
- system state model;
- system image inventory/model;
- system compatibility queries;
- system-level lifecycle state;
- system-facing management API.

### `luna-kernel-manager`

**Role:** kernel model/query backend.

Owns:
- kernel inventory/model;
- kernel metadata;
- kernel compatibility queries;
- kernel state and selection model.

Kernel installation/update/removal is executed through `luna-update-manager`.

### `luna-device-manager`

**Role:** device model and device-management backend.

Owns:
- device discovery/model;
- device state;
- device-related operations exposed to clients;
- removable-volume integration with the logical filesystem model.

It does not own application policy or security policy.

## 5. Security and state

### `luna-security`

**Role:** central policy authority.

Owns:
- permission/capability decisions;
- user/application access policy;
- administrator privilege model;
- protected user data access policy;
- recovery/factory security policy;
- policy evaluation and authorization contracts.

Low-level enforcement remains in the filesystem/kernel/runtime mechanisms.

### `luna-state`

**Role:** persistent state abstraction where a dedicated state crate is justified.

Owns:
- persistent state representation;
- state loading/storing contracts;
- durable checkpoints/metadata that are not specifically owned by boot or another subsystem.

Specialized boot metadata remains owned by the boot-state design rather than being collapsed into generic state.

### `luna-event`

**Role:** internal asynchronous event/message contracts.

Owns:
- event types;
- subscriptions/publication contracts;
- asynchronous system notification primitives.

This is a contract layer, not a Kafka dependency or Kafka clone.

## 6. Runtime

### `luna-system-runtime`

**Role:** system-level runtime and supervisor.

Owns:
- system runtime lifecycle;
- supervision of `UserSession` and application runtime instances;
- resource reservation/enforcement integration;
- runtime diagnostics and failure detection;
- coordination with system/security/device services.

There is one system runtime supervising multiple user sessions.

### `luna-user-session`

**Role:** user-session instance model.

Owns:
- user/session identity as one conceptual instance;
- session-scoped state;
- relationship between one user and that user's application runtime instances;
- session lifecycle.

The accepted hierarchy is conceptually:

```text
system-runtime
├── UserSession A
│   ├── app-runtime
│   └── GUI
└── UserSession B
    ├── app-runtime
    └── GUI
```

### `luna-app-runtime`

**Role:** application execution runtime.

Owns:
- application instance lifecycle;
- execution environment;
- process/resource isolation;
- logical-root mapping consumption;
- application runtime state;
- enforcement integration with Security;
- multi-instance rules.

It must make an application appear to run on a normal machine rather than exposing a Docker/container-style identity to the application.

## 7. Boot and early system

### `luna-boot`

**Role:** UEFI bootloader and boot-time selection/recovery logic.

Owns:
- `luna-boot.efi`;
- boot menu and boot selection;
- boot-state metadata interaction;
- system/kernel compatibility selection;
- fallback chain;
- Factory/Recovery/Emergency boot entry handling.

Boot remains independent from the normal system runtime.

### `luna-boot-state`

**Role:** dedicated persistent boot-state metadata contract/storage.

Owns:
- boot success/failure metadata;
- last-known-good information;
- failed kernel/system compatibility markers;
- state needed by the bootloader without requiring the running system.

## 8. User-facing clients

### `luna-cli`

**Role:** primary CLI client.

Owns:
- command parsing;
- aliases and user-facing command vocabulary;
- presentation of backend results.

It does not contain manager/runtime implementation logic.

The CLI may expose short aliases such as `app i`, `sys u`, `dev list`, while routing to the same backend used by GUI clients.

### `luna-gui` (future)

**Role:** graphical client.

It consumes the same backend contracts as the CLI and does not duplicate manager logic.

## 9. Logging

### `luna-log`

**Role:** logging backend/client contract.

Owns:
- structured logging interfaces;
- log routing/storage integration;
- filtering/query contracts.

Logs remain separate from the primary managers and runtimes.

## 10. Dependency direction

The intended direction is approximately:

```text
                         ┌───────────────┐
                         │   luna-cli    │
                         │   luna-gui    │
                         └───────┬───────┘
                                 │
                         backend/API contracts
                                 │
       ┌───────────────┬─────────┼──────────┬────────────────┐
       ▼               ▼         ▼          ▼                ▼
 app-manager     system-manager device-manager security   update-manager
       │               │                    │                │
       └───────────────┴──────────┬─────────┴────────────────┘
                                  ▼
                         system/app runtime
                                  │
                         root-mapping / fs
                                  │
                              Linux

Shared foundation underneath the contracts:

                         luna-common

Bundle representation:

                         luna-bundle
                              │
                         .lbp transport
```

This diagram is conceptual. Exact Cargo dependency edges are to be finalized during API-contract design so that cycles are prevented deliberately rather than discovered after implementation.

## 11. First implementation order

The first implementation wave should not create every crate at once.

1. `luna-common` — finish the audited foundation.
2. `luna-fs` — define low-level filesystem boundary.
3. `luna-root-mapping` — define logical-root mapping contracts.
4. `luna-config` — define configuration/state access contracts.
5. `luna-bundle` — define bundle internal model before `.lbp` implementation.
6. `luna-security` — define policy contracts before runtime enforcement.
7. `luna-event` — define async event contracts needed by managers/runtime.
8. Management/runtime crates follow after their API boundaries are specified.

No large implementation should begin before the public responsibility of the relevant crate is written down.

## 12. Explicitly deferred

The following are intentionally not fixed by this map yet:

- exact Cargo dependencies;
- exact IPC mechanism;
- exact async channel implementation;
- exact Linux namespace primitives;
- exact filesystem backend implementation;
- Bundle Format v1 wire/archive details;
- System Image binary format;
- kernel image format and metadata encoding;
- recovery encryption implementation;
- final GUI crate structure.

Those decisions belong to their respective API/specification work rather than being guessed during the crate-map stage.
