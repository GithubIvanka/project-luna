# Project Luna — Phase 1.2 — Decisions AA–AR

**Status:** Accepted working architecture
**Date:** 2026-08-17
**Phase:** 1.2 — Runtime, Sessions, Application Lifecycle & Recovery

This document records the AA–AR decisions as an additive continuation of the Phase 1.2 decision record. It does not silently replace earlier accepted decisions.

## AA — Hybrid application namespace model

**Decision: C — hybrid application-specific namespace model.**

Each application receives its own filesystem/mount namespace. The application sees a conventional Linux-compatible logical root, but the contents of that root are composed specifically for that application.

The namespace may contain:

- the application bundle;
- required application resources;
- required dependency files;
- permitted user data/configuration;
- required system files;
- explicitly permitted external volumes.

The namespace is therefore not a second physical Linux filesystem. It is a runtime view assembled from Luna's physical storage and policy.

## AB — File-oriented mappings and application-owned filesystem view

Mappings are **file-oriented**, not blind whole-directory mappings.

An application may receive mappings such as:

```text
/etc/app.conf → DATA/users/<user>/config/<app>/app.conf
/lib/gtk/...  → DATA/system/libs/gtk/4/...
/etc/hosts    → System Image /etc/hosts
```

The application sees conventional Linux paths, while physical resources remain in Luna's clean DATA structure.

Every application has its own filesystem view. It should appear to the application as though it is operating inside a clean Linux system in which the application's own files and explicitly permitted system resources are present.

Files belonging to unrelated applications must not be visible merely because they exist in DATA.

This is implemented through the combination of:

- application-specific filesystem namespaces;
- file-level mappings;
- visibility rules;
- permissions.

The exact distinction between visible, readable and writable resources remains part of the permission design.

## AC — Hybrid root with namespace-specific paths

The logical root remains shared conceptually, but each application namespace receives its own path composition.

For example, two applications may both see:

```text
/lib/gtk
```

while the namespace of App A resolves it to GTK 3 and App B resolves it to GTK 4.

This prevents name and dependency conflicts while preserving normal Linux paths for applications.

## AD — Permissions are part of namespace construction

Permissions are integrated with the namespace model.

An application does not receive unrestricted access merely because a physical resource exists.

The conceptual permission states are:

```text
visible
readable
writable
```

These states are configurable and policy-controlled.

The permission model and mapping model share the same architectural principle:

```text
mapping policy  = what logical resource may be composed from what physical resource
permission policy = which namespace may see/use that resource and with what access
```

## AE — Application bundles are immutable

Application bundles are immutable units regardless of where they are physically stored.

Moving an application to another disk or removable device does not move its mutable user state.

Mutable state remains associated with the user/application identity in DATA:

```text
DATA/users/<user>/data/<app>
DATA/users/<user>/config/<app>
```

The application bundle and application state are therefore separate lifecycle objects.

## AF — Orphaned application data management

The application manager is responsible not only for installation, launch, update and removal of applications, but also for identifying data that no longer belongs to an installed application.

The user should not have to manually search DATA for orphaned application data.

The application-management UI may expose:

- installed applications;
- their associated data;
- orphaned data from removed applications;
- data size;
- retention settings;
- manual deletion controls;
- protection/lock controls for data that should not be automatically removed.

Automatic cleanup policies remain configurable.

## AG — SYSTEM write protection

SYSTEM has two protection layers:

1. the normal filesystem state is read-only;
2. authorization to perform SYSTEM writes is restricted to the updater/system-management component.

Even if SYSTEM is temporarily made writable for an update or removal transaction, ordinary processes must not gain write access.

This is necessary because the updater may need to remove or replace a currently running System Image while that image is still in use.

## AH — Kernel retention and Factory separation

The updater may remove unused kernels, but must retain at least:

- the currently required kernel;
- at least the previous kernel for fallback.

**Factory Kernel is a separate immutable installation entity.**

The Factory Kernel must not be treated as an ordinary retained kernel and must not be removed, replaced or modified by normal lifecycle cleanup.

Likewise, the Factory System Image is an immutable original installation entity.

Together:

```text
Factory System Image
Factory Kernel
```

form the original known-good system state and provide the final recovery path to return the installation to its initial working state.

## AI — Recovery is a full working environment without persistent DATA

Recovery is not merely a read-only diagnostic shell.

It is a functional Luna system running with a temporary virtual recovery user whose state exists in RAM and leaves no persistent user state after reboot.

Recovery must be able to operate when normal DATA is unavailable and should contain specialized tools for:

- diagnosis;
- repair;
- removing broken/incompatible DATA components such as faulty drivers;
- restoring system functionality;
- inspecting and recovering user data;
- using external media where appropriate.

The normal runtime path is:

```text
minimal RAM root
    ↓
System Image
    ↓
attach DATA
    ↓
normal user sessions
```

If DATA cannot be used:

```text
minimal RAM root
    ↓
System Image
    ↓
Recovery user
    ↓
Recovery environment
```

If the System Image itself cannot be started, the Factory path is used.

## AJ — Factory is the immutable initial installation state

Factory consists of the original, known-good installation entities:

```text
Factory System Image
Factory Kernel
```

They are installed with Luna and are intended to be guaranteed working recovery components.

Factory is not an ordinary backup and is not part of normal version retention. It is the immutable initial state to which Luna can return when normal system versions cannot be used.

## AK — Layered lookup with per-file mappings and visibility policy

The accepted lookup model is:

```text
application
    ↓
user
    ↓
system
```

However, mappings operate on individual files rather than blindly replacing entire directories.

An application sees only the files composed into its namespace. For example, an application may see its own `/etc` configuration files plus permitted system files such as `/etc/hosts`, while files belonging to other applications remain outside its visible filesystem view.

The namespace therefore provides the illusion of a clean Linux installation dedicated to that application.

## AL — Mapping precedence

**Decision: application → user → system.**

When the same logical file can be supplied by multiple layers, the more specific layer wins:

```text
App mapping
    ↓ fallback
User mapping
    ↓ fallback
System resource
```

This preserves the previously accepted Python-like conceptual layering without implying Python semantics.

## AM — System and foreign files are not writable by applications

An application must not be able to modify:

- system files;
- another application's files;
- other users' protected files;
- resources outside its explicitly authorized writable mappings.

Seeing a logical system path does not imply write permission.

Writable resources must be explicitly provided by the namespace/mapping/permission policy.

## AN — New-file creation is policy-controlled

Applications may not freely create arbitrary files anywhere in the logical root.

Creating a file is permitted only where the namespace provides an appropriate writable mapping and permission.

For example, if `/etc/app.conf` is mapped to a writable user configuration location, writing that file is permitted. An arbitrary attempt to create a new file in an otherwise read-only system path must fail.

## AO — `/home/<user>` is the default application home

The default logical home for an application is:

```text
/home/<user>/
```

mapped to:

```text
DATA/users/<user>/home/
```

Applications do not receive access to other users' home directories by default.

A user-facing setting may later allow a different default logical home such as `/home`, but the normal/default model is the active user's `/home/<user>`.

## AP — User visibility/access states are configurable

The namespace permission model may distinguish at least:

```text
visible
readable
writable
```

The application does not automatically receive access to other users' data or unrelated application data.

The user can explicitly grant additional locations when required.

## AQ — Devices use the same permission mechanism

Device access is part of the same namespace/permission architecture.

Applications do not automatically receive unrestricted access to `/dev` or physical devices.

Conceptually:

```text
Application namespace
    ├── filesystem permissions
    ├── user-data permissions
    ├── external-volume permissions
    └── device permissions
```

The exact device permission API and user-facing permission UI remain future work.

## AR — Application Manager + separated runtime/root responsibilities

The application manager remains responsible for the application lifecycle:

- installation;
- launch;
- update;
- removal;
- application-data management.

`luna-root` remains narrowly responsible for the logical Linux-compatible root and controlled filesystem/path mapping. It must not become an application-lifecycle monolith.

The runtime/namespace functionality is separated from `luna-root` rather than being absorbed into it.

Conceptually:

```text
App Manager
    │
    ├── install
    ├── launch
    ├── update
    ├── remove
    └── data lifecycle
          │
          ▼
     Runtime / Namespace layer
          │
          ├── application namespace
          ├── permissions
          └── process lifecycle
                  │
                  ▼
              luna-root
                  │
                  └── logical root + mappings
```

The exact crate/process/API boundaries remain implementation work.

## Open implementation questions after AA–AR

The following remain intentionally open:

- exact namespace creation mechanism;
- exact kernel mount-namespace operations;
- exact file-level mapping implementation;
- exact representation of visible/readable/writable permissions;
- exact permission policy format;
- exact mechanism for hiding unrelated application files while retaining required system visibility;
- exact `/home` permission implementation;
- exact device permission implementation;
- exact runtime component name and crate/process split;
- exact interaction between App Manager and runtime namespace creation.
