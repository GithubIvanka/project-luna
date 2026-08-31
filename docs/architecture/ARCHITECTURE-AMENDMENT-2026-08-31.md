# Project Luna — Architecture Amendment 2026-08-31

Status: **Accepted amendment to `docs/ARCHITECTURE.md`**

This file records decisions accepted after Phase 1.6-HZ that are not yet physically merged into the 142 KB Source of Truth file. It is traceability material and must be read together with `docs/ARCHITECTURE.md`; where this amendment explicitly states a later decision, the later decision supersedes the older wording.

## 1. Runtime hierarchy

The architectural runtime hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

There is no separate `lunad` architectural component and no separate normal Session Manager. `UserSession` is the combined user/session entity. `luna-system-runtime` is the single system-wide runtime/supervisor.

`luna-app-runtime` owns application-instance lifecycle and depends on the system runtime. `luna-app-manager` does not own process execution; it owns installation, import, verification, update/removal procedure and migrations.

## 2. Logical root and mapping

Applications receive a normal Linux-compatible logical `/`, not a synthetic root containing only application paths.

The physical Luna storage model remains distinct from the logical root. `SYSTEM`, `DATA/system`, `DATA/users/<user>`, `DATA/cache` and approved external volumes are composed through mapping and namespace materialization.

Mapping remains primarily file-oriented, but explicit subtree/directory mappings are allowed where semantically useful (for example shared library/resource trees).

Bundle manifests declare logical mappings and bundle-relative sources. They never grant access and never expose physical `DATA/...` storage paths as application API.

The mapping table is namespace-local and normally constructed in RAM for the specific application/user/security context. Applications do not know that the mapping table exists and do not see other applications' mapping tables.

## 3. Security and mapping boundary

`luna-security` remains the central policy authority.

Manifest mapping/access/capability declarations are requests only. Effective authorization is determined by the security policy and runtime context.

The enforcement chain is:

```text
Bundle declaration
    ↓
luna-root-mapping
    ↓
luna-security authorization
    ↓
Linux namespace materialization
    ↓
ApplicationInstance
```

An enforced deny cannot be weakened by a lower-level application or instance rule.

## 4. DATA model

Canonical mutable DATA remains:

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

`DATA/system/config` contains configuration that applies to the whole machine. `DATA/users/<user>/config` contains user-scoped configuration. `DATA/system/state` is the system-owned persistent runtime/state area.

Application mutable data does not belong inside immutable `.lbp` payloads.

## 5. Persistent state

`luna-state` has a synchronous, revision-checked storage abstraction and the first durable backend is `redb`.

The default system state path is:

```text
DATA/system/state/luna-state.redb
```

Transactions and the global revision are committed atomically. A second Luna-specific WAL is not layered above the embedded database.

## 6. Update and rollback

`luna-update-manager` remains the mutation coordinator; domain managers remain owners of system/application/kernel state.

The accepted update flow is:

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

On failure, already-applied work is rolled back in reverse order. Interrupted operations are reconciled after restart using durable operation state.

`current`, `factory`, versioned System Images and compatible kernels remain independent boot concepts. System generations are not introduced merely to duplicate the existing `luna-X.Y.Z.squashfs` version model.

## 7. Linux namespace backend

`luna-namespace` is the OS-specific backend. It may use Linux mount namespaces, bind mounts, idmapped mounts, tmpfs, procfs/sysfs and other native Linux mechanisms.

The backend does not own authorization and does not replace `luna-root-mapping`.

Each ApplicationInstance receives its own filesystem/mount namespace. PID namespaces are optional implementation capabilities; an application must not be made aware of an artificial container/VM model merely because isolation is implemented.

The logical root is assembled as a conventional Linux filesystem view. Physical DATA/SYSTEM paths remain implementation details.

## 8. Devices and `/dev`

Applications do not receive unrestricted host `/dev` access.

The runtime exposes only authorized device nodes/resources. `luna-device-manager` owns device discovery and volume management; `luna-security` owns authorization; the namespace backend performs the final filtered materialization.

## 9. IPC and D-Bus

Internal Luna control-plane IPC uses Unix-domain sockets and a Luna-defined typed binary protocol with explicit API versions.

D-Bus is optional desktop compatibility infrastructure and is not the primary internal control-plane protocol. When exposed to applications it is filtered/policy-controlled rather than granting the full host system bus by default.

## 10. Application state and configuration precedence

The conceptual precedence for configuration/mutable state is:

```text
user override
    ↓
application/default content
    ↓
system default
```

The exact precedence is semantic-class-specific; not every resource class uses the same source ordering.

User customization never mutates an installed immutable Bundle payload.

## 11. Bundle Format v1 — accepted decisions

RFC-0002 is accepted as Bundle Format v1.

The accepted v1 container uses:

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

Section table entry:

```text
offset  size  field
0       4     section_type
4       4     compression
8       8     offset
16      8     compressed_length
24      8     uncompressed_length
32      32    content_hash
```

v1 section types are:

```text
1 MANIFEST   required exactly once
2 PAYLOAD    required exactly once
3 RESOURCES  optional at most once
4 SIGNATURE  optional at most once
```

`RESOURCES` is intentionally optional; v1 does not require a separate resource registry when PAYLOAD is sufficient.

## 12. Bundle identity

Bundle identity is:

```text
BundleId
Version
ContentIdentity
```

`Version` is `MAJOR.MINOR.PATCH`.

`ContentIdentity` is BLAKE3-256 over canonical manifest + canonical payload + canonical resources when present. Filename and physical location are excluded.

Different versions remain independent immutable Bundles and may coexist.

## 13. Bundle manifest

The manifest is mandatory TOML.

v1 permits bundle types:

```text
application
component
```

Application bundles require an entry point.

Manifest mappings use logical paths and bundle-relative sources. Capabilities are requests only.

Dependency references are allowed without embedding every dependency inside the same Bundle.

Unknown required semantics, unsupported enumerated values, malformed version constraints and unsafe paths must fail closed.

## 14. Deterministic payload

PAYLOAD is a deterministic TAR stream.

Canonical ordering is lexicographic by canonical Bundle-relative path.

Canonical metadata includes:

```text
uid = 0
gid = 0
mtime = 0
```

Host usernames/group names and host-specific timestamps are excluded.

Allowed payload entries in v1:

- regular files;
- directories.

Forbidden in v1:

- symlinks;
- hard links;
- FIFOs;
- sockets;
- device nodes;
- other special entries.

PAYLOAD is canonically compressed with zstd. Transport copies may use no compression for PAYLOAD/RESOURCES without changing semantic ContentIdentity.

## 15. Signature and trust

The optional signature section uses Ed25519.

The signed material is canonical bundle content/metadata and excludes the signature section itself.

A valid signature does not itself mean the Bundle is trusted or authorized.

The broader supply-chain model may additionally authenticate repository metadata; repository metadata is outside the `.lbp` container.

Unsigned Bundles are format-valid. Whether they may be installed or launched is a trust-policy decision.

## 16. Installation

The accepted safety boundary is:

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

No partially registered Bundle may remain after failed installation.

## 17. Recovery and boot

Recovery is a separate Recovery System Image/runtime mode with temporary RAM-backed state for the recovery user/context. It is not Factory.

Factory is the original known-good installed System Image + kernel combination and remains separate from Recovery.

The normal Boot Menu remains:

```text
normal boot
system/kernel selection
recovery
factory
```

`Emergency` is a health/diagnostic state, not an additional normal Boot Menu entry.

Boot state changes on events rather than every successful boot. A successful new boot may record a boot-success/health confirmation so the system can distinguish "booted" from "validated healthy" for update rollback decisions.

## 18. Resource protection

The system maintains protected resource budget for continued operation of system-runtime, diagnostics and security infrastructure.

Application CPU, memory, process-count, I/O and other limits are enforced through available Linux mechanisms such as cgroups v2 where appropriate. Specific resource policies remain configurable/adaptive rather than hardcoding one percentage for every machine.

## 19. Current implementation boundary

The following implementation pieces exist:

```text
luna-root-mapping       domain mapping primitives
luna-namespace          Linux namespace backend
luna-security           policy boundary
luna-state              memory + redb persistence
luna-update-manager     checkpointed update coordinator
luna-bundle             LBP1 reader/writer candidate
luna-app-runtime        instance lifecycle + namespace preparation
luna-boot               separate UEFI project, currently booting kernel + test init + sh
```

These implementations must continue to be checked against this amendment and the main Source of Truth before being considered production-ready.

## 20. Supersession rule

Where an older section of `docs/ARCHITECTURE.md` says that `.lbp` is not yet specified, that statement is superseded by RFC-0002 Accepted.

Where older sections use `.img` for System Images, the current naming invariant remains `luna-X.Y.Z.squashfs` and System Image means the SquashFS filesystem itself.

Where older sections use `system/core`, the current directory name is `system/kernels/`.

Where older sections describe the previous `data/` structure, the current canonical DATA model is the structure in section 4 above.

# END OF AMENDMENT