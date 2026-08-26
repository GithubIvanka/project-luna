# Project Luna — Phase 1.2 — Decisions R–Z

**Status:** Accepted working architecture  
**Date:** 2026-08-16  
**Phase:** 1.2 — Runtime, Sessions, Application Lifecycle & Recovery

This document records decisions R–Z from the Phase 1.2 architecture discussion. It is an additive decision record and must not silently override earlier accepted decisions.

## R — Responsibility of `luna-root`

**Decision: C — keep `luna-root` focused on logical root/path mapping.**

`luna-root` is the component responsible for constructing the logical Linux-compatible root and applying controlled namespace/path mappings. It should not absorb unrelated application-lifecycle or session-management responsibilities merely because it participates in runtime setup.

The architecture therefore prefers separation of responsibilities:

- `luna-root` — logical root and mapping composition;
- application/runtime components — application namespace lifecycle;
- session/user components — user session state and switching;
- updater/system-management components — updates and system-image lifecycle;
- recovery/factory components — recovery paths.

The exact process/API boundaries remain implementation work.

## S — Lifetime and eviction of application namespace state

**Decision: configurable + adaptive memory-pressure behavior.**

Application namespace state and mapping tables are not deleted immediately when an application exits. They may remain in RAM so a subsequent launch can reuse the existing namespace composition.

Retention is not a fixed hard-coded timeout. The system should support:

1. a user/system-configurable retention policy;
2. automatic memory-pressure eviction;
3. eviction from oldest/least-recently-used inactive application state toward recently closed applications as RAM becomes necessary;
4. keeping state as long as practical when sufficient RAM is available.

The previously discussed ~1 hour value is therefore only an example, not a final default.

The system should reclaim enough RAM rather than indiscriminately clearing every retained namespace.

## T — Location of mapping tables

**Decision: RAM, not persistent DATA.**

Per-application mapping tables are runtime state. They should normally live in RAM and be reconstructed when necessary from the application's declared requirements, user configuration, system metadata and policy.

The table is therefore not another persistent directory/file structure that the user has to manage.

Persistent application configuration/data remains in the appropriate DATA locations; the runtime mapping table itself is ephemeral state, although the runtime may retain it in RAM after application exit according to the S policy.

## U — Namespace architecture

**Decision: C — application-specific namespaces remain the primary isolation boundary.**

Each application gets its own filesystem/mount namespace. The namespace is composed from explicitly permitted application, user and system resources plus any explicitly authorized external volumes.

This preserves the Phase 1.1 model:

```text
application layer
       ↓
user layer
       ↓
system layer
```

The mapping table is local to the namespace. There is no global table shared by all applications.

This also allows incompatible dependency versions to coexist without forcing a single global library version.

## V — Dependency acquisition / missing resources

**Decision: D + B — do not silently pull arbitrary dependencies; locate, explain, and ask.**

Luna should not automatically download every missing dependency without the user's knowledge or consent.

When an application requires something that is not currently available, the system should:

1. determine what is required;
2. determine whether a suitable local/resource-repository version exists;
3. explain the requirement to the user;
4. show where the required resource can be obtained when appropriate;
5. ask whether the user wants it downloaded/installed.

The exact distinction between mandatory system dependencies, optional application dependencies and repository-provided resources remains to be designed.

## W — Permissions and mapping policy

**Decision: unified policy concept.**

The permission model and the path-mapping model should use the same underlying architectural principle: an application is not allowed to arbitrarily reach any physical DATA path or map it into any logical Linux path.

Examples:

- `DATA/users/<user>/config` may satisfy an approved configuration path such as logical `/etc`.
- `DATA/users/<user>/config/bin` must not automatically become logical `/bin` merely because the path exists.
- Access to user data, configuration, and external volumes must be explicitly permitted according to policy.

Thus:

```text
mapping policy = what logical resource may be composed from what physical resource
permission policy = which namespace is authorized to receive/use it
```

The two systems should be designed together rather than as unrelated permission mechanisms.

## X — Per-user behavior when switching sessions

**Decision: all three behaviors are supported, configurable per user.**

Each user can independently configure what happens to their applications when their session stops being the active desktop session:

1. applications continue running normally;
2. applications remain alive but are restricted/limited;
3. applications are terminated.

The default behavior remains **option 2 (restricted)**.

This is effectively a per-user fourth configuration model composed from the first three options. User A may choose behavior 1 while User B chooses behavior 3.

The exact definition of “restricted” remains an implementation/design task and may involve application suspension, resource throttling, reduced external-volume access, or other controls.

## Y — Failure handling philosophy

**Decision: graceful diagnosis and repair before panic.**

Luna should avoid treating failures as an immediate unrecoverable desktop-killing event.

When a serious failure occurs, the preferred progression is:

```text
failure detected
      ↓
diagnose
      ↓
try safe recovery / repair
      ↓
notify user with useful information
      ↓
if unresolved → enter an explicit emergency state
```

Even in the emergency state, Luna should preserve user agency where technically possible. The user should be able to save data, connect external media, inspect the situation, and use recovery/repair tools rather than being presented with a dead-end “system crashed” screen.

The system should provide meaningful notifications and diagnostic information instead of merely exposing a generic panic/failure message.

The exact emergency-state implementation, diagnostic tooling and safety boundaries remain future work.

## Z — Checkpoint / rollback semantics

**Decision: hybrid checkpoint architecture; user-selectable, not a mandatory full-DATA snapshot.**

Btrfs snapshots are treated as **checkpoints/recovery points**, not as runtime session-switching machinery and not as a substitute for conventional user-data backups.

The user can configure the checkpoint subsystem using the previously accepted choices between option 2 and option 3, or disable it completely. The default is option 2.

The architecture explicitly does **not** require taking a snapshot of the entire DATA partition for every checkpoint.

Snapshots remain visible and manageable by the user.

The conceptual distinction is:

```text
System Image rollback
    → immutable OS version recovery

Btrfs checkpoint/rollback
    → recovery of selected mutable DATA state

Future user-data backup system
    → long-term backup of user data
```

A future backup subsystem may be added later, but it is not part of the current Phase 1.2 checkpoint architecture.

## Resulting Phase 1.2 runtime model

The accepted direction now forms the following conceptual model:

```text
                 luna-boot.efi
                       ↓
                minimal RAM root
                       ↓
                 System Image
                 ↙           ↘
          DATA available    DATA unavailable
               ↓                  ↓
        local user sessions   Recovery user
               ↓                  ↓
      application namespaces   recovery tools
               ↓
      per-namespace mappings
               ↓
      application lifecycle
```

Btrfs checkpoints operate alongside this runtime as a recovery facility; they do not implement normal session switching.

## Still intentionally open after R–Z

- exact `luna-root` API/process boundary;
- exact namespace creation and teardown API;
- exact definition of Restricted session state;
- exact memory-pressure eviction algorithm;
- exact mapping-table in-memory representation;
- exact dependency discovery/download protocol;
- exact permission policy language and enforcement mechanism;
- exact diagnostic/emergency state;
- exact Btrfs checkpoint scope for options 2 and 3;
- checkpoint naming/retention/automatic creation rules;
- exact interaction between updater transactions and user/session state.
