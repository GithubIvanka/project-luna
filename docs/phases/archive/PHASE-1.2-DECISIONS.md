# Project Luna — Phase 1.2 Decisions

**Status:** Accepted working decisions  
**Phase:** 1.2 — Runtime, sessions, users, recovery and lifecycle  
**Date:** 2026-08-16

This document records decisions accepted during Phase 1.2. It supplements, but does not silently replace, the Architecture Source of Truth.

## 1. Boot/runtime initialization order

Luna should establish a **minimal RAM-based logical root first**.

Conceptually:

UEFI / kernel → minimal RAM root → system image → DATA → normal user session

DATA must not be attached before the minimum environment required to continue safely exists.

Reason: if DATA fails, Luna should already be in a state close to Recovery rather than having performed additional initialization that depends on DATA.

## 2. DATA failure

If DATA cannot be attached or made usable during normal boot, Luna enters **Recovery mode**.

Recovery mode uses the OS without the normal user DATA environment. The goal is to allow repair and then reboot into normal user mode.

If the OS/System Image itself cannot be started, Luna falls back to **Factory mode**.

Conceptual hierarchy:

Normal → Recovery → Factory

with each level requiring less mutable state than the previous one.

## 3. Multiple simultaneous users

Luna supports multiple users being active simultaneously. User sessions are independently managed rather than treating the machine as having only one global interactive user session.

## 4. User session lifecycle

A user can choose what happens when leaving their session:

1. Keep the session active.
2. Keep it active but restricted.
3. Fully terminate the session.

The default should be option 2: the session remains available but is restricted.

Different users may independently have different session states.

## 5. Applying configuration changes

A configuration change should not automatically imply a full system reboot.

Preferred lifecycle:

configuration change → determine affected state → restart/re-enter the affected user session when sufficient → reboot only when genuinely required

Some system services may continue running across user changes. For example, an updater may continue an already-running update transaction while user A leaves and user B becomes active.

## 6. Application isolation

Every application receives its own mount/filesystem namespace. The namespace contains only resources required by that application and permissions granted to it.

This provides the foundation for layered path resolution:

1. application layer
2. user layer
3. system layer

This is conceptually similar to layered lookup, but Luna's implementation must remain explicitly controlled by namespace policy rather than becoming unrestricted path rewriting.

## 7. Per-namespace mapping tables

Path mappings are not stored in one universal global table. Each application namespace receives a small mapping table containing only the mappings required by that namespace.

Example:

Application A:
`/libs/gtk` → `DATA/system/libs/gtk/3`

Application B:
`/libs/gtk` → `DATA/system/libs/gtk/4`

Inside each namespace the application sees only its own `/libs/gtk`.

## 8. Mapping policy

Mappings are rule-controlled. Luna must not allow arbitrary physical DATA paths to overwrite arbitrary Linux paths.

Example: `/etc` → user's configuration area can be an allowed class of mapping.

A path such as `/users/<user>/config/bin` must not automatically become an arbitrary executable/system path merely because it exists.

The mapping system therefore needs explicit path classes and allowed mapping relationships.

## 9. Persistent namespace state

Application runtime state should not be recreated unnecessarily on every application restart.

At minimum, the following may remain associated with an application namespace:

- its mapping table;
- relevant runtime metadata;
- other small state needed for efficient recreation.

The state may remain in RAM for a managed lifetime after the application exits. An example discussed was roughly one hour if the application is not running, after which memory can be released. The exact timeout is not final.

## 10. External storage and application access

Applications do not receive unrestricted access to all mounted volumes by default. Access is granted according to permissions/policy.

The user should be able to decide which locations/resources an application may access.

## 11. Checkpoint / rollback subsystem

Btrfs snapshots are considered useful for a dedicated **checkpoint/rollback subsystem**.

This is not a requirement to snapshot all DATA continuously.

The user should be able to choose between:

- option 2 — a more targeted checkpoint model;
- option 3 — a broader checkpoint model;

and be able to disable the feature completely.

Default: option 2.

The user must be able to see and work with the technology rather than having snapshots hidden as an opaque implementation detail.

The exact scope of each snapshot mode remains to be specified.

## 12. Important distinction

Btrfs snapshots are a DATA-management/recovery mechanism.

They do not replace:

- immutable System Images;
- Factory fallback;
- kernel fallback;
- application namespace isolation.

They complement those mechanisms.

## 13. Still unspecified

The following remain design questions rather than accepted implementation details:

- exact RAM-root implementation;
- exact hybrid SquashFS loading mechanism;
- exact Recovery boot protocol;
- exact Factory boot protocol;
- exact session manager;
- exact namespace lifetime implementation;
- exact mapping-table format;
- exact allowed mapping classes;
- exact application permission model;
- exact Btrfs checkpoint granularity and retention;
- exact behavior when a user's session is restricted;
- exact update transaction semantics across user switches.

These must be designed before implementation.
