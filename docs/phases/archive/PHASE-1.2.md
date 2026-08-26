# Project Luna — Phase 1.2: Runtime Lifecycle, Sessions & Recovery

Status: ACCEPTED (implementation details intentionally deferred)

## Scope

Phase 1.2 defines the runtime lifecycle around:

- minimal RAM root initialization;
- hybrid System Image loading;
- System Image / DATA attachment order;
- Recovery and Factory modes;
- simultaneous user sessions;
- user session lifecycle;
- application namespaces;
- per-namespace path mappings;
- application permissions;
- Btrfs checkpoint/rollback;
- persistence and lifetime of runtime mapping state.

## 1. Boot-to-runtime order

The accepted high-level sequence is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
minimal RAM root
  ↓
System Image (hybrid / lazy loading)
  ↓
attach DATA
  ↓
normal runtime
```

The important ordering rule is:

> Establish the minimum viable RAM root and System Image environment before attaching DATA.

This avoids attaching DATA and performing user-data setup when the selected System Image cannot start. If DATA cannot be attached after the system itself is viable, the system can transition directly toward Recovery.

The exact low-level implementation is not yet specified.

## 2. Hybrid System Image loading

The selected System Image remains immutable SquashFS, but Luna uses a hybrid loading model:

- the logical root is represented in RAM / virtual filesystem;
- only the minimum required content is made available initially;
- immutable SquashFS blocks are loaded lazily as required;
- the whole System Image does not need to be copied into RAM at boot.

The goal is therefore:

```text
logical Linux /
        ↓
RAM / virtual FS
        ↓
required SquashFS blocks on demand
```

This preserves the user's desired model: physically there is no ordinary Linux root directory tree on the System partition, while the kernel and applications see the normal Linux filesystem layout they require for compatibility.

## 3. DATA failure and Recovery

If DATA cannot be attached or initialized:

```text
minimal RAM root
      ↓
System Image
      ↓
DATA attach FAIL
      ↓
Recovery mode
```

Recovery is a usable Luna system, not merely a bootloader screen.

Recovery starts the OS without the normal persistent DATA environment. It uses a virtual recovery user whose data lives in RAM rather than on DATA.

Therefore Recovery can operate even when DATA is unavailable.

Conceptually:

```text
Normal mode:
System Image + DATA + local users

Recovery:
System Image + RAM recovery user
```

This allows the user to inspect and repair problems that prevent DATA from loading, then reboot into normal mode.

## 4. System Image failure and Factory

If the selected System Image itself cannot be started, normal Recovery based on that System Image is insufficient.

The fallback chain is therefore:

```text
selected System Image fails
        ↓
Factory System Image
        ↓
Factory Kernel (compatible factory boot path)
        ↓
Factory mode
```

Factory remains the final local mechanism for obtaining a bootable Luna environment when ordinary system images are unusable.

Factory Image and Factory Kernel are separate artifacts.

## 5. Multiple simultaneous users

Luna supports multiple users being active simultaneously.

A user session is not required to terminate merely because another user logs in.

Each user has an independent session state:

```text
ACTIVE
RESTRICTED
TERMINATED
```

Default behavior when a user leaves the active desktop session is:

```text
ACTIVE → RESTRICTED
```

The user may configure the departure behavior to:

1. leave the session active;
2. restrict the session;
3. fully terminate the session.

Different users may independently use different states.

## 6. System services survive user changes

User-session lifecycle is deliberately separated from system-service lifecycle.

A system service does not have to stop merely because the active desktop user changes.

Example:

```text
User A
  ↓
starts update
  ↓
User A leaves
  ↓
User B logs in
  ↓
update service continues
```

The system may later notify User B that the update is ready to be applied.

This is particularly relevant to services such as the updater and other system-level background tasks.

## 7. Application namespaces

Each application receives its own filesystem namespace.

```text
Application A
    ↓
namespace A

Application B
    ↓
namespace B
```

The namespace is assembled from explicitly permitted layers:

```text
application
    ↓
user
    ↓
system
```

This is related to the mapping architecture defined in Phase 1.1, but the runtime owns the concrete namespace instance.

An application does not automatically receive access to the complete DATA or host filesystem.

## 8. Namespace-local mapping tables

Every application namespace has its own mapping table.

There is intentionally no single global table containing every mapping for every application.

A mapping table is small and contains only mappings required by that namespace.

Example:

```text
Application A namespace:
/lib/gtk → DATA/system/libs/gtk/3/...

Application B namespace:
/lib/gtk → DATA/system/libs/gtk/4/...
```

Both applications can therefore see the logical path `/lib/gtk` without conflicts.

The mapping operation is file-oriented: Luna maps the required logical files rather than blindly mapping entire physical directories. This is important because logically related files may originate from different physical locations.

Mapping remains subject to explicit policy. A namespace must not be able to arbitrarily map any DATA path into any Linux path.

## 9. Namespace mapping lifetime

Namespace mapping state should not necessarily be destroyed immediately when an application exits.

The accepted direction is to retain the application's runtime mapping state for a limited period so that a restart can reuse it without reconstructing the complete mapping table every time.

Conceptually:

```text
application exits
      ↓
namespace state retained temporarily
      ↓
application restarts
      ↓
reuse mapping state
```

If the application does not return within the configured retention period, the associated runtime state can be released and its RAM reclaimed.

The exact retention duration and eviction policy are not yet specified.

## 10. Application permissions

Applications do not receive unrestricted access to DATA by default.

Access is granted explicitly through the namespace/runtime permission model.

The user should be able to decide which user locations and external volumes an application may access.

The intended model is therefore:

```text
application
    ↓
minimal permissions
    ↓
user grants additional access when required
```

The detailed permission UI and enforcement protocol remain future work.

## 11. Checkpoint / rollback

Btrfs snapshots are used as a user-facing recovery mechanism, not as the mechanism that implements normal runtime session switching.

The subsystem is conceptually a:

**checkpoint / rollback subsystem**

It is configurable by the user.

The accepted options are the previously discussed user-selectable checkpoint modes, excluding a snapshot of the entire DATA partition as a mandatory model.

The default is option 2.

The feature can be completely disabled.

The user should be able to work with this mechanism rather than having it hidden from them.

Snapshot boundaries, naming, retention, atomicity requirements, and exact option-2 semantics remain to be specified in a later phase.

## 12. Runtime principle

The central runtime architecture is:

```text
                luna-boot.efi
                      ↓
               minimal RAM root
                      ↓
              System Image runtime
                ↙          ↘
        DATA available     DATA unavailable
             ↓                    ↓
       local users           Recovery user
             ↓                    ↓
      normal runtime       recovery runtime
```

The key invariant is that the operating system itself must be capable of reaching a useful Recovery environment without relying on the persistent user DATA environment.

## 13. Explicitly deferred details

The following are intentionally not finalized in Phase 1.2:

- exact initramfs implementation;
- exact RAM-root construction mechanism;
- exact SquashFS lazy-loading backend;
- exact DATA attach implementation;
- exact namespace creation API;
- exact mapping-table format;
- mapping rule language / policy format;
- namespace state persistence format;
- mapping-state eviction timeout;
- application permission protocol and UI;
- Btrfs snapshot boundaries;
- checkpoint retention policy;
- exact Recovery repair tooling;
- exact Factory boot implementation.

These should be specified only when their implementation phase is reached.
