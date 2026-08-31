# Project Luna — Architecture Source of Truth

**Project:** Project Luna  
**Internal name:** `luna`  
**Implementation language:** Rust  
**License:** Apache License 2.0  
**Kernel:** Linux  
**Document status:** Architectural Source of Truth  
**Current architecture baseline:** Phase 1.6-HZ + accepted decisions through 2026-08-31

---

## 1. Architecture purpose

This document is the authoritative description of the current Project Luna architecture.

Accepted decisions are recorded here as the current architecture. Historical phase records, ADRs and RFCs provide traceability; they do not form a second active architecture specification.

The architecture is implemented as a set of narrow components with explicit ownership boundaries. Implementation details may evolve without changing those boundaries.

---

## 2. Core principles

Project Luna is an immutable Linux-based operating system built around a small stable system foundation, versioned System Images, independently versioned kernels and self-contained application Bundles.

The core principles are:

1. System Images are immutable and versioned.
2. Kernel versions are independent from System Image versions.
3. User and application mutable state is kept outside immutable System Images and immutable installed Bundles.
4. The physical storage layout is Luna-native and compact.
5. Applications receive isolated execution environments assembled from explicitly authorized resources.
6. Linux mechanisms are used as implementation primitives rather than exposed as the architecture itself.
7. Updates are transactional and recoverable.
8. Security policy, mapping semantics and namespace materialization are separate responsibilities.
9. Persistent state is durable and revisioned.
10. The boot path remains small, stable and focused on boot selection and handoff.

---

## 3. Physical storage model

A Luna installation consists of four physical areas:

```text
EFI
SYSTEM
DATA
SWAP
```

`SWAP` is optional and may be implemented as a partition, file or ZRAM.

`EFI` and `SYSTEM` are OS-managed. `DATA` is the normal mutable/user-visible storage area.

### 3.1 EFI

The EFI area contains the Luna bootloader:

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

### 3.2 SYSTEM

`SYSTEM` contains versioned immutable OS images and kernels:

```text
SYSTEM/
├── images/
│   ├── luna-<version>.squashfs
│   └── luna-<version>.toml
└── kernels/
    └── <version>/...
```

A System Image is the SquashFS filesystem itself. The canonical filename is:

```text
luna-X.Y.Z.squashfs
```

The matching manifest is:

```text
luna-X.Y.Z.toml
```

The System Image is not a Bundle and is not represented by `.lbp`.

### 3.3 DATA

The canonical mutable DATA layout is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`DATA/system/apps` stores installed application Bundles shared between users.  
`DATA/system/drivers` stores OS-managed mutable driver content.  
`DATA/system/libs` stores shared library content.  
`DATA/system/volumes` stores managed external-volume state.  
`DATA/system/config` stores machine-wide mutable configuration.  
`DATA/system/state` stores persistent system state.  
`DATA/users/<user>/home` is the user's ordinary home directory.  
`DATA/users/<user>/data` stores application/user mutable data.  
`DATA/users/<user>/config` stores user-scoped configuration.  
`DATA/cache` stores disposable cache data.

The first durable `luna-state` backend uses:

```text
DATA/system/state/luna-state.redb
```

### 3.4 Multi-disk installations

EFI/SYSTEM and DATA/SWAP may reside on different physical disks. The installer may recommend layouts according to available capacity, retained System Images and kernels.

---

## 4. Boot architecture

The normal boot path is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
selected compatible kernel
  ↓
minimal RAM logical-root environment
  ↓
selected System Image
  ↓
attach DATA
  ↓
luna-system-runtime
  ↓
UserSession(s)
```

`luna-boot.efi` is intentionally small and focused on UEFI boot flow, selection, validation and kernel handoff.

The normal boot path is quiet and does not present a persistent boot menu.

### 4.1 Boot menu

The boot menu is entered by pressing `B` during boot.

It provides:

- System Image selection;
- compatible kernel selection;
- Recovery;
- Factory recovery;
- external-media boot;
- other boot/recovery operations supported by the platform.

### 4.2 Boot state

Boot state changes only for relevant events. An ordinary successful boot does not rewrite boot state merely because the machine booted.

A successful boot may record a health/confirmation event so the system can distinguish a booted generation from a validated healthy generation when making update rollback decisions.

Retention counts are configurable. The factory state is retained independently.

---

## 5. System Images

A System Image is a directly usable SquashFS filesystem image containing one version of the immutable Luna userspace/system environment.

The canonical representation is:

```text
luna-X.Y.Z.squashfs
```

Each image has an adjacent manifest:

```text
luna-X.Y.Z.toml
```

The manifest provides boot metadata such as image identity, version, architecture and compatible kernel information. The exact final manifest schema is an implementation/specification task, but the per-image manifest model is accepted.

System Images are immutable during normal operation.

### 5.1 System Image loading

The logical Linux root is RAM-based and can expose System Image content lazily/hybrid rather than requiring the complete SquashFS payload to be copied into RAM up front.

The exact kernel/SquashFS implementation is platform-specific and remains an implementation detail.

---

## 6. Kernel model

Kernels are independent versioned system resources stored under:

```text
SYSTEM/kernels/
```

System Image and kernel updates are independent. A new System Image does not require replacing the kernel, and a kernel update does not require rewriting the System Image.

The boot selector resolves only compatible System Image/kernel combinations.

The current kernel is never removed during ordinary lifecycle management.

Factory recovery consists of a retained factory-good System Image and a retained factory kernel.

---

## 7. Logical Linux root

Luna presents applications and system components with a conventional Linux-compatible logical `/` while keeping the physical Luna storage model separate.

The logical root is assembled from controlled sources including:

```text
System Image content
DATA-backed mutable state
application resources
user state
approved external volumes
```

The logical root is a composition layer, not a mirrored physical copy of the DATA hierarchy.

`luna-root-mapping` owns mapping semantics and mapping plans. The active runtime receives validated mapping state rather than modifying an active mapping table in place.

The accepted mapping model is:

```text
application
    ↓
user
    ↓
system
```

Mappings are primarily file-oriented. Explicit subtree/directory mappings are allowed where they are semantically appropriate, such as shared library/resource trees.

A mapping plan is immutable after validation. A policy change that invalidates a plan requires revalidation before the plan can be used again.

---

## 8. Application isolation and namespaces

Every `ApplicationInstance` receives its own filesystem/mount namespace.

The namespace contains only resources needed by that application and explicitly authorized for its user/session context.

The application sees a normal Linux-compatible logical root. Physical `DATA/...` and `SYSTEM/...` locations, mapping-table representation and other storage implementation details are hidden behind the Luna composition layer.

`luna-namespace` is the Linux-specific materialization backend. It may use native mechanisms including mount namespaces, bind mounts, idmapped mounts, tmpfs, procfs, sysfs, cgroups and related kernel facilities.

PID namespaces are an optional implementation mechanism. Their use does not change the application-facing Luna model.

Namespace mapping state is normally held in RAM for the relevant application/user/security context and may be retained after application exit according to adaptive policy.

---

## 9. Security and authorization

`luna-security` is the central policy authority.

Security policy is revisioned. Authorization decisions may be bound to a policy snapshot/revision.

The permission model separates visibility from operations:

```text
Visibility
Read
Write
Execute
Use
Manage
```

Grants may be operation-scoped, one-time, while-running or persistent where appropriate.

Trust records are content-specific and bind at least:

```text
BundleId
ContentIdentity
trust scope
```

Trusting one content version does not automatically trust a later modified version of the same application identity.

Manifest mappings, capabilities and access declarations are requests. They are not authorization grants.

The enforcement chain is:

```text
Bundle declaration
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
Linux namespace materialization
    ↓
ApplicationInstance
```

An effective deny cannot be weakened by a lower-level instance or application rule.

---

## 10. Application model

Applications are self-contained immutable Bundles presented in a macOS-like directory model.

Installed Bundles live under:

```text
DATA/system/apps/
```

A Bundle may be shared by multiple users. Different application versions may coexist independently.

Mutable application state is stored outside the Bundle:

```text
DATA/users/<user>/data/<app>/
DATA/users/<user>/config/<app>/
```

The default logical application home is:

```text
/home/<user>/
```

An application Bundle can be used by multiple UserSessions without changing the Bundle itself.

Bundles may be portable across disks/media; mutable user state remains independent of the physical Bundle location.

`luna-app-manager` owns installation, import, verification, update, removal and migration procedures. It constructs and validates application plans; it does not own application process execution.

A valid `ApplicationPlan` is prevalidated for dependencies, compatibility, authorization, resource requirements and migration requirements before execution.

---

## 11. Bundle Format v1 (`.lbp`)

RFC-0002 — Bundle Format v1 is accepted.

`.lbp` is the transport/archive representation of a Luna Bundle. It is separate from System Images.

### 11.1 Container structure

The v1 container begins with:

```text
LBP1
└── fixed 64-byte header
    └── fixed 64-byte section entries
```

All integer fields are little-endian.

Header:

```text
offset  size  field
0       4     magic = LBP1
4       2     major version = 1
6       2     flags = 0 for v1
8       4     section_count
12      8     section_table_offset
20      8     section_table_length
28      4     header_length = 64
32      32    header_hash = BLAKE3(header with hash bytes zeroed)
```

Section entry:

```text
offset  size  field
0       4     section_type
4       4     compression
8       8     offset
16      8     compressed_length
24      8     uncompressed_length
32      32    content_hash
```

Section types:

```text
1 MANIFEST   required exactly once
2 PAYLOAD    required exactly once
3 RESOURCES  optional at most once
4 SIGNATURE  optional at most once
```

### 11.2 Bundle identity

A Bundle is identified by:

```text
BundleId
Version
ContentIdentity
```

`Version` uses SemVer `MAJOR.MINOR.PATCH`.

`ContentIdentity` uses BLAKE3-256 over canonical manifest, payload and optional canonical resources. Filename and physical location are excluded from identity.

### 11.3 Manifest

The Bundle manifest is mandatory TOML.

v1 supports:

```text
application
component
```

Application Bundles require an entry point.

Manifest mappings contain logical paths and Bundle-relative sources. Physical DATA paths are not part of the Bundle-facing mapping API.

Capabilities are requests, not grants.

Dependencies may be referenced without embedding every dependency into the same Bundle.

### 11.4 Payload

`PAYLOAD` is a deterministic TAR stream.

Canonical path ordering is lexicographic by Bundle-relative path.

Canonical metadata includes:

```text
uid = 0
gid = 0
mtime = 0
```

v1 payload entries are regular files and directories. Symlinks, hard links and special filesystem entries are not part of v1 payloads.

PAYLOAD uses canonical zstd compression. Transport-level compression choices do not change semantic Bundle ContentIdentity.

### 11.5 Signatures and trust

The optional signature section uses Ed25519.

The signed material covers canonical Bundle content/metadata and excludes the signature section itself.

Signature validity, trust and authorization are separate concepts.

Unsigned Bundles are format-valid. Whether an unsigned Bundle may be installed or launched is determined by trust/security policy.

### 11.6 Installation boundary

The accepted installation sequence is:

```text
read
  ↓
validate header
  ↓
validate section table
  ↓
verify section hashes
  ↓
parse + validate manifest
  ↓
validate payload
  ↓
verify ContentIdentity
  ↓
verify signature when present
  ↓
Security/trust decision
  ↓
stage
  ↓
atomic commit
```

An installation failure must not leave a partially registered Bundle.

---

## 12. Runtime hierarchy

The system-wide runtime hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

`luna-system-runtime` is the single system-wide runtime/supervisor.

`UserSession` is the combined user/session entity.

`luna-app-runtime` owns application-instance lifecycle and execution-environment preparation.

`luna-app-manager` owns application lifecycle procedures rather than process supervision.

---

## 13. ApplicationInstance lifecycle

An `ApplicationInstance` has an explicit lifecycle:

```text
Starting
  ↓
Running
  ↓
Stopping
  ↓
Stopped
```

A running instance may enter `Failed` from an unrecoverable runtime failure.

The instance identity is runtime state. It does not belong in the immutable Bundle manifest.

Similarly, UserSession identity is runtime state and does not belong in the Bundle manifest.

---

## 14. Users and sessions

Multiple users may have simultaneous active sessions.

Each user's session may be:

```text
ACTIVE
RESTRICTED
TERMINATED
```

When a user leaves the active desktop session, the default behavior is `RESTRICTED`.

The user may configure application/session behavior independently:

- continue normally;
- remain alive but restricted;
- terminate.

System services may continue while users switch sessions. An update transaction may continue independently from the client UI/session when safe.

Configuration changes should affect only the relevant session/services where practical; reboot is reserved for changes that genuinely require it.

---

## 15. External volumes and devices

Luna provides automatic, user-friendly external volume handling.

The user interacts with volume labels and file-manager entries rather than raw Linux device paths.

Managed external volume state is represented under:

```text
DATA/system/volumes/<friendly-name>
```

Applications do not receive unrestricted host `/dev` access.

The device flow is:

```text
discovery
   ↓
luna-device-manager
   ↓
luna-security authorization
   ↓
filtered namespace materialization
```

`luna-device-manager` owns device discovery and volume management. Security owns authorization. Namespace/runtime performs final filtered exposure.

---

## 16. Configuration and mutable state

Configuration is selected according to semantic resource class. The general model is:

```text
user override
    ↓
application/default content
    ↓
system default
```

System defaults remain immutable in the System Image. User-specific changes live in DATA.

An installed Bundle is not a storage location for persistent mutable user state.

---

## 17. Persistent state

`luna-state` owns the persistent state domain model and storage abstraction.

Its first durable backend is `redb`:

```text
DATA/system/state/luna-state.redb
```

The state domain is backend-independent and is not Btrfs-specific.

Transactions and the global revision are committed atomically in the durable backend.

`luna-state` may store references to checkpoints/operations, but checkpoint/snapshot internals remain a separate subsystem.

No second Luna-specific WAL is layered above the embedded database.

---

## 18. Updates, checkpoints and rollback

`luna-update-manager` is the mutation coordinator.

Domain managers remain owners of their own state:

```text
luna-system-manager  → System Image state/query model
luna-kernel-manager  → kernel state/query model
luna-app-manager     → application lifecycle model
```

The accepted transaction sequence is:

```text
prepare
  ↓
checkpoint
  ↓
apply
  ↓
verify
  ↓
commit
```

Before commit, old state remains authoritative wherever the individual operation can provide that guarantee.

If an operation terminates after mutation has started, reconciliation determines whether it committed, partially committed or did not commit.

Failure does not imply automatic rollback.

Rollback is an explicit recovery/transaction action and applies already-defined recovery semantics in reverse order where supported.

Interrupted operations retain durable operation state sufficient for restart-time reconciliation.

Closing the CLI/GUI does not cancel a backend operation merely because the client disconnected.

---

## 19. Btrfs checkpoints

Btrfs snapshots are a checkpoint/rollback subsystem for selected mutable DATA state.

They are not the mechanism for normal runtime session switching and are not the System Image rollback mechanism.

The user can select a targeted checkpoint scope, broader scope or disable checkpoints. The default is the targeted option.

Exact checkpoint scope, naming, retention and automatic-creation policy remain implementation/specification work.

---

## 20. Recovery and Factory

Recovery is a functional Luna environment using a System Image and temporary RAM-backed recovery state.

Normal recovery from DATA failure is:

```text
minimal RAM root
    ↓
System Image
    ↓
Recovery context
```

The recovery context uses temporary state and does not require the user's normal persistent session state.

Recovery can support diagnosis, repair, removal/disablement of broken DATA components, user-data recovery and external-media access.

Factory is separate from Recovery. Factory is the retained known-good installation pair:

```text
Factory System Image
Factory Kernel
```

Factory state is retained independently of normal lifecycle operations.

The boot system respects System Image/kernel compatibility when selecting previous, current or factory combinations.

---

## 21. Events and IPC

`luna-event` provides correlated event delivery and event-history semantics.

Events carry enough correlation information to associate them with operations and relevant source/target context.

Live subscriptions and persistent event history are separate concerns.

Subscriptions have an explicit lifecycle, such as subscribe → active → cancelled/completed.

The internal control-plane IPC model uses Unix-domain sockets with a Luna-defined typed binary protocol and explicit API versions.

D-Bus is optional desktop compatibility infrastructure. It is not the primary internal Luna control-plane protocol, and application exposure is filtered/policy-controlled.

---

## 22. Resource control

The runtime maintains protected resources for system runtime, security and diagnostics.

Application resource limits may cover CPU, memory, process count, descriptors, I/O and related resources using native Linux mechanisms such as cgroups v2 where appropriate.

Policies are configurable/adaptive and are not defined by one universal percentage for every machine.

---

## 23. Component ownership

The principal userspace boundaries are:

```text
luna-common
    shared domain-neutral primitives

luna-fs
    low-level filesystem primitives and filesystem errors

luna-root-mapping
    logical/physical mapping semantics, MappingTable, MappingPlan

luna-namespace
    Linux namespace and logical-root materialization

luna-config
    configuration model and parsing

luna-security
    policy, grants, trust and authorization

luna-state
    persistent state domain + storage abstraction/backends

luna-event
    event delivery/history and correlated operation events

luna-bundle
    immutable Bundle domain + LBP1 representation/codec

luna-app-manager
    application planning, install/import/verify/update/remove/migrate

luna-system-manager
    System Image state and query model

luna-kernel-manager
    kernel inventory/compatibility/query model

luna-update-manager
    mutation coordination, checkpoints, verification and rollback

luna-device-manager
    device discovery and volume management

luna-system-runtime
    system-wide supervision and runtime orchestration

luna-user-session
    UserSession state and lifecycle

luna-app-runtime
    ApplicationInstance lifecycle and execution-environment preparation

luna-cli
    thin user-facing control interface
```

The bootloader is maintained separately under `boot/luna-boot/` and is outside the userspace Cargo workspace architecture.

---

## 24. Filesystem domain rules

`luna-fs` provides low-level filesystem concepts including files, directories, metadata, symlink handling and filesystem mode/permission metadata without deciding authorization policy.

`luna-fs` owns its own filesystem error types. There is no global catch-all filesystem error enum in `luna-common`.

`PhysicalPath` belongs to mapping/storage layers rather than becoming a generic cross-domain primitive.

`luna-root-mapping` exposes typed logical/physical path concepts at its domain boundary instead of exposing raw host paths directly to applications.

`MappingTable` supports insertion, removal, lookup, conflict detection and validation.

Validated mapping tables are immutable while active. Updating mapping state produces a new table/version that can replace the active table atomically.

---

## 25. Security revision and trust invariants

Security decisions are evaluated against a defined policy revision/snapshot.

A grant has an explicit scope and lifetime. A previous successful authorization is not treated as an unconditional permanent grant.

Trust is bound to Bundle content identity, not only to the human-readable application identity.

The Bundle, mapping, security and runtime layers never collapse into one shared authorization mechanism.

---

## 26. Implementation and specification state

The following are established architecture boundaries with implementation work continuing behind them:

```text
System Image manifest schema                 OPEN
Kernel metadata schema                       OPEN
Exact SYSTEM filesystem                      OPEN
Boot-state persistent schema                 OPEN
Installer placement algorithm                OPEN
Hybrid SquashFS/RAM implementation           OPEN
Final luna-root API                          OPEN
Final logical path-class taxonomy            OPEN
Final namespace state/retention policy       OPEN
Final application permission API             OPEN
Exact device backend                         OPEN
Checkpoint scope/retention                   OPEN
Final service-manager integration            OPEN
End-to-end QEMU/Linux validation             OPEN
```

These are implementation/specification tasks within the accepted architecture, not alternative architecture proposals.

---

## 27. Development model

The project follows:

```text
Architecture
    ↓
RFC / specification
    ↓
Domain interfaces
    ↓
Prototype
    ↓
Implementation
    ↓
Integration
    ↓
Validation
```

Components should remain narrow, explicit and independently testable.

Important runtime identities such as `ApplicationInstanceId` and `UserSession` state belong to runtime state, not immutable Bundle metadata.

---

## 28. Current architecture baseline

The accepted architecture consolidated here includes:

- storage and boot decisions from Phases 1.0–1.4;
- Phase 1.6 decisions 41–75;
- Phase 1.6-HZ architecture decisions;
- post-1.6 accepted runtime, mapping, security, state, update, device, IPC and resource-control decisions through 2026-08-31;
- RFC-0002 / Bundle Format v1 acceptance.

The authoritative current semantics are the contents of this document.

Historical decision files remain useful for traceability and the formal RFC remains normative for the LBP1 wire format where this document summarizes it.
