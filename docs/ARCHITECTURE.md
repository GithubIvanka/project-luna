# Project Luna — Architecture Source of Truth

**Project:** Project Luna  
**Internal name:** `luna`  
**Language:** Rust  
**License:** Apache License 2.0  
**Kernel:** Linux  
**Status:** current architectural Source of Truth  
**Current baseline:** Phase 1.6 + accepted decisions through 2026-08-31  

`docs/ARCHITECTURE.md` is the single current architecture specification. `docs/decisions/ACCEPTED-DECISIONS.md` is the compact accepted-decision ledger. Phase records and archives preserve history and rationale.

---

## 1. Project model

Project Luna is a Linux-based operating system with a small stable immutable foundation, versioned System Images, independently versioned kernels, immutable application Bundles and mutable user/application state outside those immutable artifacts.

The system uses Linux facilities as implementation primitives while keeping the user-facing and domain architecture Luna-specific.

Development language and tooling:

```text
Rust
Cargo
Tokio where asynchronous execution is needed
```

The architecture favors narrow components with explicit ownership and minimal cross-component knowledge.

---

## 2. Physical installation

A Luna installation consists of four areas:

```text
EFI
SYSTEM
DATA
SWAP
```

`EFI` and `SYSTEM` are OS-managed. `DATA` is the normal mutable/user-visible area. `SWAP` is optional and may be a partition, file or ZRAM.

EFI/SYSTEM and DATA/SWAP may be placed on different physical disks.

### 2.1 EFI

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

### 2.2 SYSTEM

`SYSTEM` contains versioned immutable System Images and kernels.

```text
SYSTEM/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   └── luna-X.Y.Z.toml
└── kernels/
    └── <version>/...
```

A System Image is directly a SquashFS filesystem image. Its adjacent TOML manifest contains image metadata and compatible kernel information.

### 2.3 DATA

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

`DATA/system/apps` stores installed Bundles shared between users.  
`DATA/system/drivers` stores mutable driver content.  
`DATA/system/libs` stores shared library content.  
`DATA/system/volumes` stores managed external-volume state.  
`DATA/system/config` stores machine-wide mutable configuration.  
`DATA/system/state` stores persistent system state.  
`DATA/users/<user>/home` stores ordinary user files.  
`DATA/users/<user>/data` stores mutable application/user data.  
`DATA/users/<user>/config` stores user-scoped configuration.  
`DATA/cache` stores disposable cache content.

---

## 3. Boot architecture

The boot path is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
selected compatible kernel
  ↓
Luna early/boot environment
  ↓
selected System Image
  ↓
logical root + DATA
  ↓
luna-system-runtime
  ↓
UserSession(s)
```

Normal boot is quiet. The boot menu is entered by pressing `B` during the boot window.

Boot menu functions:

```text
normal/current boot
System Image selection
compatible kernel selection
Recovery
Factory
external-media boot
```

Boot state is separate from System State and Recovery State. It changes on relevant events rather than on every ordinary boot.

### 3.1 Fallback

Boot selection always respects image/kernel compatibility.

The fallback chain is conceptually:

```text
current
  ↓
compatible previous image/kernel choice
  ↓
other compatible image/kernel choice
  ↓
Factory
```

A compatible previous image may be attempted without reboot when technically possible. A kernel transition may require reboot.

### 3.2 Factory

Factory is the original known-good System Image plus the original known-good factory kernel. It is retained independently of ordinary retention cleanup.

### 3.3 Recovery

Recovery is a separate Recovery System Image.

Recovery provides:

```text
Recovery System Image
        ↓
RAM logical root
        ↓
temporary recovery identity
        ↓
diagnostic / repair tools
```

Recovery may operate without normal user DATA. Recovery writable state is RAM-backed and disappears on reboot unless a future recovery design explicitly persists selected state.

Access to protected user DATA requires explicit authorization/password according to the security model.

---

## 4. System Images and kernels

System Images and kernels are independent versioned resources.

System Image:

```text
luna-X.Y.Z.squashfs
```

Kernel:

```text
SYSTEM/kernels/<version>/...
```

The System Image manifest describes, at minimum:

```text
image identity
version
architecture
format
compatible kernels
boot metadata
```

Kernel compatibility is evaluated independently from application Bundle compatibility.

The concrete final System Image manifest schema and exact kernel metadata schema remain implementation/specification work unless separately accepted.

System Image retention is configurable. The current and usable previous state remain available according to policy, with Factory always retained.

---

## 5. Logical Linux root

Applications and system components use a conventional Linux-compatible logical `/`.

The physical Luna layout is an implementation detail of that logical root.

The logical root may compose content from:

```text
System Image
DATA/system
DATA/users/<user>
application Bundle resources
approved external volumes
approved devices
runtime pseudo-filesystems
```

The logical root is composed in the execution environment rather than represented as a physical mirror of DATA.

System Image content may be exposed lazily/hybrid. Required system content can be materialized into RAM when necessary.

If the active System Image is removed after being made unnecessary as a persistent source, required materialized system content already held in RAM remains available when no alternative source exists.

---

## 6. Root mapping

`luna-root-mapping` owns logical mapping semantics and validated mapping plans.

A Bundle can declare logical mappings such as:

```text
/usr/bin/example
/usr/lib/example.so
/etc/example.conf
```

while physical Luna storage locations remain internal.

### 6.1 Mapping granularity

File mappings are the default.

Explicit subtree/directory mappings are supported where semantically appropriate, such as shared resource or library trees.

### 6.2 Mapping scope

Mapping state is constructed for the application/user/security context and is normally held in RAM.

Identical immutable mapping definitions may be reused by multiple compatible ApplicationInstances without making mutable state global.

### 6.3 Mapping precedence

Precedence is semantic-class-specific.

Where the resource class supports layered defaults, the common model is:

```text
user
  ↓
application
  ↓
system
```

Application and user layers may override lower defaults when permitted by the resource class and security policy.

### 6.4 Mapping validity

A mapping plan is validated before execution. Conflicting mappings in one namespace are errors. An active ApplicationInstance does not mutate its accepted mapping table in place; a change creates a new validated mapping state and may require policy revalidation.

---

## 7. Application Bundles

Applications are immutable macOS-like Bundles.

Installed Bundles are stored under:

```text
DATA/system/apps/
```

A Bundle contains its executable/resources/dependencies/metadata and may declare mappings and requested capabilities.

Bundle versions are independent immutable resources and may coexist.

Portable/external Bundles may be launched from removable media or another disk after inspection, integrity verification, trust evaluation and authorization.

Bundle identity uses:

```text
BundleId
Version
ContentIdentity
```

`ContentIdentity` is independent of filename and physical location.

Mutable application state is outside the Bundle:

```text
DATA/users/<user>/data/<app>/
DATA/users/<user>/config/<app>/
```

Default configuration remains in immutable Bundle/system content and is overridden by mutable user configuration where that resource class permits it.

Application data may be cleaned manually or automatically according to retention policy. Data can be marked retained/locked against automatic cleanup.

Removing a Bundle may prompt the user about keeping its data. Automatic cleanup follows retention rules rather than behaving as a silent uninstall side effect.

---

## 8. Bundle Format v1 — `.lbp`

RFC-0002 — Bundle Format v1 is accepted.

`.lbp` is the transport/archive representation of a Luna Bundle. It is distinct from a System Image and from installed Bundle storage.

### 8.1 LBP1 container

All integer fields are little-endian.

Header size: **64 bytes**.

```text
offset  size  field
0       4     magic = LBP1
4       2     version_major = 1
6       2     flags = 0 in v1
8       4     section_count
12      8     section_table_offset
20      8     section_table_length
28      4     header_length = 64
32      32    header_hash = BLAKE3(header with hash bytes zeroed)
```

Section entry size: **64 bytes**.

```text
offset  size  field
0       4     section_type
4       4     compression
8       8     offset
16      8     compressed_length
24      8     uncompressed_length
32      32    content_hash
```

Sections:

```text
MANIFEST   required exactly once
PAYLOAD    required exactly once
RESOURCES  optional at most once
SIGNATURE  optional at most once
```

Structural validation covers magic, version, flags, bounds, overflow, overlap and truncation.

### 8.2 Manifest

Manifest format: TOML.

v1 Bundle types:

```text
application
component
```

Application Bundles have an entry point.

Manifest supports:

```text
identity
metadata
platform
entry
mapping
capabilities
optional dependencies
```

Mapping sources are Bundle-relative paths. Mapping logical paths are Linux-style absolute logical paths.

Capabilities and access declarations are requests only.

### 8.3 Payload

PAYLOAD contains a deterministic TAR representation.

Canonical payload properties:

```text
lexicographic Bundle-relative path ordering
uid = 0
gid = 0
mtime = 0
no host-specific owner/group names
```

v1 payload entries are regular files and directories.

Symlinks, hard links and special filesystem nodes are excluded from v1.

PAYLOAD uses zstd as the canonical compression policy. Manifest and signature sections remain uncompressed.

### 8.4 Content identity

Canonical content identity is:

```text
BLAKE3-256(
    canonical manifest
    || canonical payload
    || canonical resources when present
)
```

Filename, physical location, transport padding and signature bytes are excluded from ContentIdentity.

### 8.5 Signatures

Ed25519 is the accepted Bundle signature algorithm.

The optional signature covers canonical Bundle metadata/content while excluding the signature section itself.

Signature validity, trust and authorization remain separate decisions.

Unsigned Bundles are format-valid; install/launch policy is determined by trust/security policy.

Publisher and repository authenticity may both participate in the broader supply-chain model.

---

## 9. Security and authorization

`luna-security` is the central policy authority.

Permission dimensions:

```text
Visibility
Read
Write
Execute
Device Use
Manage
```

Policy is revisioned.

Grants may be:

```text
one-time
operation-scoped
while-running
persistent
```

An application-level restriction applies to all its instances. An instance may tighten policy but cannot weaken an enforced deny.

`Ask` means the operation requires explicit confirmation from the user/system client. Security does not depend on GUI implementation.

`Constrained` is structured and typed.

Manifest mappings and capabilities never bypass security authorization.

### 9.1 Administrative authority

Luna has no permanent root user and does not require `sudo`/`su` as an architectural privilege layer.

Administrative authority belongs to the configured administrator identity and is protected by an administrator password.

An administrator password cannot be removed while it is required to safely downgrade the last administrator to an ordinary user.

Password-recovery support via a recovery key/token is part of the accepted security direction.

### 9.2 Data protection

User DATA may remain unencrypted by default. The architecture supports optional whole-DATA encryption and optional per-user DATA encryption.

---

## 10. Linux namespaces and application isolation

Every ApplicationInstance receives an isolated filesystem/mount namespace.

The application sees a conventional Linux root while physical Luna paths and mapping representation remain hidden.

The initial backend uses Linux mechanisms including:

```text
mount namespaces
bind mounts
OverlayFS where useful
tmpfs
procfs
sysfs
idmapped mounts where useful
```

Additional namespaces are policy-driven:

```text
user
PID
network
IPC
UTS
time
```

PID namespace isolation is used for process isolation and supervision; applications are not modeled as containers and do not need to regard themselves as PID 1.

Applications receive only explicitly authorized devices and volumes.

No host-level administration capability is granted to ordinary applications by default.

---

## 11. Device and external-volume model

`luna-device-manager` owns device discovery and volume lifecycle.

Expected UX:

```text
USB/device connected
        ↓
discovery
        ↓
filesystem detection
        ↓
automount
        ↓
volume appears in file manager
```

Managed volume state is represented under:

```text
DATA/system/volumes/<friendly-name>
```

The file manager exposes the volume through the Luna user model rather than requiring raw Linux mount paths.

Read/write behavior is subject to user/security policy.

USB autorun is confirmation-based or disabled according to system policy; media insertion does not silently execute an application.

Network volumes may use the same abstraction in a future extension.

---

## 12. UserSession and runtime hierarchy

User and session are one domain object: `UserSession`.

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

There is no separate session manager domain.

`luna-app-runtime` owns application-instance lifecycle and execution-environment preparation.

`luna-app-manager` manages installation/import/update/removal/verification/migration and package ingestion; normal process execution is owned by runtime.

Multiple UserSessions may coexist. When a user leaves the active desktop session, the default session behavior is `RESTRICTED`. Per-user/session configuration may instead keep applications alive normally or terminate them.

---

## 13. ApplicationInstance lifecycle

`ApplicationInstanceId` is system-wide unique and is assigned under `luna-system-runtime` authority.

A Bundle may have multiple instances when its policy allows it.

Lifecycle:

```text
Starting
   ↓
Running
   ↓
Stopping
   ↓
Stopped
```

A runtime failure may move an instance to:

```text
Failed
```

and an actual process crash is represented distinctly from failure to start/recover.

If `luna-app-runtime` fails, `luna-system-runtime` may restart it without automatically destroying the UserSession. Runtime metadata can be recovered independently of process memory.

By default, closing an application releases its runtime resources. Optional policy may retain selected warm resources for faster relaunch.

Repeated crashes trigger policy-driven diagnostics and user-visible choices such as diagnosis, restart, rollback or close.

---

## 14. Resource management

The system reserves protected CPU, memory and GPU capacity for system/runtime/diagnostics/security work.

The reservation is adaptive rather than a single fixed percentage.

Linux resource-control primitives provide enforcement. `cgroups v2` is the accepted initial direction.

Memory-pressure reclamation proceeds through reclaimable resources before application termination while protected system-critical resources remain available.

Resource policy covers, where useful:

```text
CPU
memory
GPU
process count
file descriptors
persistent storage
```

Process/file-descriptor limits are protection mechanisms against resource exhaustion. Persistent DATA quotas may be introduced through policy without changing Bundle semantics.

---

## 15. Persistent state

`luna-state` owns the logical persistent state model and storage abstraction.

Persistent system state is stored under:

```text
DATA/system/state/
```

The first durable backend is `redb`.

State operations are synchronous at the storage abstraction level. Asynchronous orchestration belongs to higher layers.

State supports:

```text
atomic transaction
revision
optimistic concurrency
reopen/recovery
```

The storage backend is separate from checkpoint/rollback storage.

---

## 16. Events and operations

Events and operations are separate domain concepts.

Events have independent `EventId` and operation correlation where applicable.

Within an operation, event sequence is monotonic. Timestamp is metadata rather than ordering authority.

Event classes:

```text
Ephemeral
Persistent
Audit
```

Persistent and Audit events may be replayed/queryable according to policy.

Delivery uses bounded queues/backpressure. Audit events are never silently dropped.

Operations:

```text
view
cancel
resume
rollback
force-stop
```

Force-stop is separate from cooperative cancellation and requires stronger/emergency authorization plus user warning where applicable.

An operation belongs to System or UserSession context rather than GUI/CLI process lifetime.

Interrupted operations are reconciled after runtime/service restart.

---

## 17. Update and rollback

`luna-system-manager` owns the system state model/query boundary.

`luna-kernel-manager` owns kernel inventory, metadata and compatibility queries.

`luna-app-manager` owns application install/import/update/remove/verify/migrate procedures and can ingest `.deb` and `.rpm` packages by analyzing their Linux filesystem layout and producing a Luna Bundle and manifest/mapping description.

`luna-update-manager` executes state-changing update transactions across domains.

Update stages:

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

Interrupted work is reconciled from durable operation state.

Rollback restores the last valid checkpoint/authoritative state according to policy.

Updates are user-visible and may offer actions such as retry, diagnose, cancel, rollback or recovery.

Application versions remain independent. A newer major version may coexist with the previous version so incompatible extensions/configuration can continue to work.

Delta update is supported as a future/update transport mechanism and remains separate from `.lbp` format.

Deleting the currently active System Image requires ensuring its required runtime content is materialized or otherwise available before deletion.

Btrfs snapshots are the accepted checkpoint implementation direction where the filesystem supports them.

---

## 18. Configuration

TOML is the preferred human-readable configuration/metadata format where suitable.

Machine-wide configuration lives in:

```text
DATA/system/config/
```

User-scoped configuration lives in:

```text
DATA/users/<user>/config/
```

The common configuration lookup model is:

```text
user override
    ↓
application/default content
    ↓
system default
```

The exact precedence remains semantic-class-specific.

System-wide settings such as networking live at system scope and therefore survive user switching.

---

## 19. IPC and clients

`luna-cli` is the main CLI client.

GUI and CLI are thin clients over shared backend APIs.

The accepted IPC direction is a small versioned Unix-socket protocol with structured/binary messages. The exact wire schema remains an implementation task.

CLI aliases are configurable. A canonical operation may have short forms, for example:

```text
app install <bundle>
app -i <bundle>
sys update
sys -u
dev list
```

Aliases are user-configurable and accompanied by descriptions.

The CLI supports machine-readable output in addition to human-readable output.

D-Bus access, where required, uses filtered/limited interfaces. Wayland is the display integration direction. PipeWire is the audio/media backend.

---

## 20. Managers and crate responsibilities

```text
luna-common
    small cross-cutting value types

luna-fs
    low-level filesystem primitives

luna-root-mapping
    logical path/mapping semantics

luna-namespace
    Linux namespace/materialization primitives

luna-config
    configuration model and scoped configuration

luna-security
    policy, authorization and trust

luna-bundle
    Bundle domain, manifest, validation and .lbp codec

luna-state
    persistent state domain and storage abstraction

luna-event
    event domain, subscriptions and delivery contracts

luna-app-manager
    application install/import/update/remove/verify/migrate

luna-system-manager
    system state model and queries

luna-update-manager
    state-changing update execution

luna-device-manager
    device and volume discovery/lifecycle

luna-kernel-manager
    kernel inventory and compatibility queries

luna-system-runtime
    system-wide runtime/supervision and UserSession orchestration

luna-user-session
    UserSession domain contract

luna-app-runtime
    ApplicationInstance lifecycle and execution environment

luna-cli
    thin CLI client
```

`luna-boot.efi` is a separate boot project under `boot/luna-boot/`.

---

## 21. Security/integrity chain

The production-oriented chain is:

```text
UEFI Secure Boot
        ↓
luna-boot verification
        ↓
verified kernel/system image
        ↓
System Image / Bundle integrity
        ↓
fs-verity where supported
        ↓
Bundle signature / publisher trust
        ↓
luna-security policy
        ↓
namespace/resource enforcement
        ↓
ApplicationInstance
```

Ed25519 is the accepted Luna Bundle signature algorithm. Key rotation, revocation and trusted-key management are part of the production security architecture.

IMA integration is an accepted future/production hardening layer. TPM measured boot is optional future hardening.

---

## 22. Current implementation state

The repository contains working foundations/prototypes for:

```text
luna-boot
luna-root-mapping
luna-namespace
luna-security
luna-state + redb
luna-event
luna-bundle + LBP1
luna-update-manager
luna-app-runtime contracts
```

`luna-boot` has demonstrated loading the Linux kernel and reaching a test init and shell in its dedicated development work.

The main userspace workspace uses the accepted crate boundaries above. Some components remain contract-level or integration-level implementations and require further backend work before production readiness.

---

## 23. Development rules

The development order is:

```text
Architecture
    ↓
RFC/specification
    ↓
interfaces
    ↓
prototype
    ↓
implementation
    ↓
integration
    ↓
tests/verification
```

A crate does not absorb another crate's responsibility for convenience.

`luna-common` remains small.

Accepted architectural decisions are not silently changed by implementation work.

If implementation reveals a genuine architectural conflict, it must be reported before the Source of Truth is changed.

### 23.1 Rust learning rule

Luna development doubles as a Rust learning environment. Important Rust constructs are explained when they matter to the implementation, including:

```text
ownership and borrowing
struct / enum
Result / Option
traits
generics
lifetimes
modules and crates
async/await and Tokio
unsafe code when present
```

The implementation should prefer clear and teachable Rust over clever abstractions when both satisfy the architecture.

---

## 24. Phase status

```text
Phase 1.1 — accepted
Phase 1.2 — accepted
Phase 1.3 — accepted
Phase 1.4 — accepted
Phase 1.5 — accepted
Phase 1.6 — accepted through the recorded decision set and subsequent accepted clarifications
RFC-0002 Bundle Format v1 — accepted
```

Current work is implementation hardening and integration on top of this architecture.
