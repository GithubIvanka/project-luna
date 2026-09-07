# Project Luna — Logical Root and Application Mapping Model

**Status:** architecture direction accepted through discussion on 2026-09-07
**Branch:** `develop`

This document clarifies the already established logical-root and application mapping model. It does not replace `docs/ARCHITECTURE.md`; it records the detailed semantics needed to prevent accidental drift during implementation.

## 1. User-facing filesystem model

An application receives a conventional Linux-compatible logical filesystem tree:

```text
/
├── bin
├── dev
├── etc
├── home
├── lib
├── proc
├── sys
├── tmp
├── usr
└── var
```

The application is not expected to know that the tree is assembled by Luna from multiple sources. Physical SYSTEM/DATA paths and mount mechanics are implementation details.

## 2. The logical root is not a physical copy

The logical `/` is RAM-backed runtime storage composed from controlled sources. It is not the physical SYSTEM partition, and the selected SquashFS System Image is not itself the long-lived application root.

Conceptually:

```text
System base
    + authorized application resources
    + authorized user data
    + approved volumes/devices
    + runtime pseudo-filesystems
          ↓
       logical /
```

The composition may use OverlayFS and/or individual controlled mounts depending on the resource class and security requirements. The architecture does not require one mechanism for every mapping class.

## 3. OverlayFS role

OverlayFS is a candidate/approved Linux primitive for composing filesystem layers where its semantics fit the resource class. OverlayFS combines upper/lower directory trees and presents a merged view. It is particularly useful for layering an application-specific tree over an existing Linux-compatible root. The kernel documentation confirms that matching directory names are merged and that an upper object hides the corresponding lower object. citeturn643330search0

OverlayFS must **not** be interpreted as permission filtering by itself. If a full `/etc` exists in the lower tree, merely adding an upper layer does not automatically hide every file in the lower `/etc`; directory contents are merged unless explicitly hidden. Therefore application security must not depend on a generic assumption that mounting an upper `/etc` makes unrelated lower files invisible. citeturn643330search0

## 4. Partial mapping of directories

Luna does not grant an application an entire sensitive directory merely because it needs one file from that directory.

For example, an application may need:

```text
/etc/my-app.conf
/etc/resolv.conf
```

but must not automatically receive all of:

```text
/etc/*
```

The mapping contract therefore operates at file granularity by default. Explicit subtree/directory mappings are allowed only where semantically justified and authorized. This matches the existing root-mapping decision that file mappings are the default granularity and directory/subtree mappings are explicit exceptions.

When only selected files are required, the final logical environment must expose only those selected resources through mapping/materialization and security policy. Other sensitive files must either be absent from the application's view or be inaccessible under the enforced security model.

## 5. Mapping is not authorization

The application requests logical resources; `luna-root-mapping` determines a deterministic `MappingPlan`; `luna-security` decides whether each requested resource may be exposed with specific permissions; only then may `luna-namespace` materialize the approved result.

```text
Application declaration
        ↓
ApplicationPlan
        ↓
luna-root-mapping
        ↓
MappingPlan
        ↓
luna-security
        ↓
AuthorizedApplicationPlan
        ↓
luna-namespace
        ↓
logical application /
```

A mapping declaration never grants access by itself.

## 6. Visibility and access are separate

Luna distinguishes at least:

```text
Visibility
Read
Write
Execute
Device Use
Manage
```

Therefore a path can be visible while a specific operation is denied, or it can be omitted from the application's logical view entirely.

Example:

```text
/etc
    visible as a normal Linux directory

/etc/my-app.conf
    visible + read allowed

/etc/secret-system-file
    absent or access denied
```

The exact combination for any path is determined by the authorized policy and materialization result.

## 7. System configuration example

A network-aware application may be allowed to read selected networking configuration without receiving the rest of `/etc`:

```text
/etc/resolv.conf        → allow read
/etc/hosts              → policy-dependent
/etc/my-app.conf        → allow read/write if authorized
/etc/shadow             → deny
/etc/ssh/*              → deny unless explicitly required
```

This is an example of the model, not a frozen universal allowlist. The system policy remains authoritative.

## 8. Application isolation goal

The application should experience the logical environment as an ordinary Linux installation while its actual view is individually composed and constrained.

Conceptually:

```text
Application A
    sees its logical /
    + its authorized resources
    + its authorized user/session resources

Application B
    sees its logical /
    + a different authorized resource set
```

The two applications may observe the same standard Linux path names while those names resolve to different backing resources.

## 9. Security mechanisms

Mount namespaces establish a private filesystem/mount view. Landlock and other kernel security controls can further constrain filesystem operations. These mechanisms are complementary, not interchangeable. The Linux kernel documentation notes that OverlayFS and bind mounts have different security and object semantics, and that Landlock rules operate on filesystem hierarchies as actually encountered by a task. citeturn643330search1

Production materialization may use modern file-descriptor-based mount APIs (`openat2`, `open_tree`, `mount_setattr`, `move_mount`) to bind trusted sources to explicitly controlled logical destinations. `open_tree(... OPEN_TREE_CLONE)` creates a detached mount object that can be modified and then attached with `move_mount`, which is useful for avoiding fragile pathname-only workflows. citeturn451613search1turn451613search3

## 10. User identity model

Luna's user-facing identity model does not expose a permanent `root` account or require `sudo`/`su` as the administrative hierarchy.

The principal user roles are:

```text
admin
user
guest
```

Administrative authority is an explicit Luna security concept rather than a second permanent login user. If the sole administrator account is downgraded to an ordinary user, Luna requires a separate administrator credential/password to regain administrative authority; this administrator credential must not be empty and must not equal that user's ordinary password.

Linux UID/GID and capabilities remain kernel implementation mechanisms. They must not be mistaken for Luna's user-facing account model.

## 11. Non-goals

This model does not mean:

- every application gets a complete private copy of `/`;
- every directory is mounted recursively for every application;
- `/etc` as a whole is automatically granted because one configuration file is needed;
- OverlayFS alone is the security boundary;
- a PID namespace or application PID-1 supervisor is required;
- physical SYSTEM/DATA paths become visible to applications.

## 12. Architectural invariant

The user-facing abstraction is:

> **A normal Linux logical `/`, individually composed and security-filtered for each execution environment.**

The implementation may use OverlayFS, bind mounts, detached mounts, RAM-backed storage, Landlock and other Linux primitives as appropriate, but those mechanisms must remain behind the logical mapping/security abstraction.
