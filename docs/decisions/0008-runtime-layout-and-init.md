# Decision 0008 — Runtime Layout and Init Contract

**Status:** ACCEPTED  
**Date:** 2026-09-01  
**Related:** `2026-09-01-LIBC-AND-INIT.md`, `2026-09-01-RUNTIME-INTEGRATION.md`, RFC-0002

## Decision

Project Luna adopts a two-stage userspace startup and a per-process runtime model:

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux kernel
 ↓
luna-init
 ↓
logical root
 ↓
luna-system-runtime (PID 1)
 ↓
UserSession
 ↓
luna-app-runtime
 ↓
ApplicationInstance
 ↓
runtime environment
```

The system libc is musl. glibc is an optional, Luna-managed compatibility runtime and is never installed as the global system libc.

## 1. Runtime classes

A process may request one of three logical runtime classes:

```text
luna
 glibc
 bundle
```

### `luna`

The native Luna runtime. System services and Luna components are built and executed against musl.

### `glibc`

An approved, versioned glibc compatibility runtime. It is materialized only in namespaces that require it.

### `bundle`

A self-contained runtime supplied by the Bundle when the Bundle contract permits it. Its libraries remain private to the application namespace.

Runtime selection is policy-controlled. A manifest declaration is a request and is not authorization.

## 2. Physical storage

The canonical physical storage remains:

```text
SYSTEM/
└── images/
    └── luna-X.Y.Z.squashfs

DATA/
└── system/
    ├── apps/
    ├── libs/
    └── ...
```

Runtime storage is an implementation detail. In particular, applications MUST NOT receive mappings that expose physical `SYSTEM/...` or `DATA/...` paths.

A future implementation may organize managed compatibility runtimes under a dedicated runtime subtree of `DATA/system`, but the physical directory name is not part of the application ABI.

## 3. Logical library view

The application always receives a logical Linux filesystem.

Conceptually:

```text
native process:
    /lib* → approved Luna/musl runtime

compatibility process:
    /lib* → approved glibc runtime

self-contained process:
    /lib* → Bundle-private runtime
```

The exact `lib`, `lib64`, loader and architecture-specific paths are determined by the ELF/runtime requirements and the approved mapping plan. Luna MUST NOT rely on a single hard-coded `/lib/libc.so` replacement scheme.

## 4. One libc environment per process

A normal process has exactly one selected libc environment.

Supported:

```text
process A → musl
process B → glibc
process C → Bundle-private libc
```

Not supported as a general runtime model:

```text
one process → interchangeable musl + glibc libc
```

Different processes may use different libc environments concurrently on the same Luna installation.

## 5. Mapping and namespace sequence

Runtime materialization follows the existing Luna security chain:

```text
Bundle/runtime declaration
        ↓
resolve runtime identity/version
        ↓
luna-root-mapping validation
        ↓
luna-security authorization
        ↓
namespace materialization
        ↓
ELF/runtime validation
        ↓
exec
```

Runtime selection MUST resolve to a Luna-managed or explicitly approved source. An application cannot request an arbitrary host library directory.

`luna-root-mapping` owns logical mapping semantics. `luna-namespace` owns Linux namespace materialization. Runtime owns execution lifecycle.

## 6. `luna-init` contract

`luna-init` is early userspace, not the permanent service manager.

Its responsibilities are deliberately narrow:

1. consume boot-selected System Image state;
2. discover and validate SYSTEM;
3. locate the selected SquashFS System Image;
4. construct the initial logical root;
5. attach DATA according to the boot/runtime contract;
6. prepare required `/dev`, `/proc`, `/sys` and related early userspace state;
7. transfer control to the permanent `luna-system-runtime`.

`luna-init` MUST NOT become a general service supervisor, application manager or package manager.

The handoff target is the permanent Luna system runtime once the normal logical root is ready.

## 7. `luna-system-runtime` contract

`luna-system-runtime` is the permanent PID 1 and sole system-wide process supervisor.

It owns:

- service lifecycle;
- dependency ordering;
- readiness/state tracking;
- restart policy;
- child process ownership;
- signal and lifecycle handling;
- resource supervision;
- UserSession orchestration;
- coordination with `luna-app-runtime`.

The design is intentionally closer to runit/OpenRC in scope and philosophy than to systemd, but it is a Luna-specific implementation and API.

`luna-app-runtime` MUST NOT introduce a competing process supervisor.

## 8. Application execution boundary

`luna-app-runtime` prepares an `ApplicationInstance` execution environment and asks the system runtime for process lifecycle operations according to the existing contract.

The sequence is:

```text
ApplicationInstance
 ↓
resolve Bundle/runtime
 ↓
resolve dependencies
 ↓
build mapping plan
 ↓
security decision
 ↓
materialize namespace
 ↓
start process through system-runtime
 ↓
monitor lifecycle
```

Application identity, Bundle identity and process/PID identity remain distinct.

## 9. Compatibility runtime versioning

glibc compatibility runtimes are independently versioned artifacts. Updating glibc MUST NOT change the libc used by Luna system components.

An application may pin or constrain a compatible glibc runtime according to a future runtime manifest contract. Runtime garbage collection MUST preserve any runtime still referenced by a running or retained application state.

## 10. Boot/runtime failure policy

Failure before the permanent system runtime is established belongs to the early-boot/recovery path.

Failure after `luna-system-runtime` owns PID 1 is handled by system-runtime supervision and the existing recovery/operation policy.

`luna-init` failure MUST NOT be hidden as an ordinary service restart because no permanent supervisor exists yet.

## 11. Security invariants

- Physical storage paths are never application-visible by default.
- Runtime declarations are requests, not grants.
- Security authorization precedes namespace materialization.
- Unknown/unapproved runtime identities fail closed.
- A glibc runtime cannot grant additional device, filesystem or network capabilities.
- A runtime choice cannot weaken an existing application/system deny.
- Library mappings cannot escape their approved runtime source.
- An application cannot mutate its accepted runtime/mapping state in place; a changed environment requires a new validated state.

## 12. Implementation order

The implementation should proceed in this order:

```text
1. define typed RuntimeIdentity / RuntimeClass model
2. define runtime resolution contract
3. connect runtime resolution to luna-root-mapping
4. materialize musl-native namespace
5. materialize versioned glibc namespace
6. add ELF interpreter/runtime validation
7. harden luna-init handoff
8. replace bring-up shell path with luna-system-runtime
9. implement service graph + supervision
10. integrate application lifecycle
```

The first implementation milestone is intentionally small: a musl-native bootable userspace reaching `luna-system-runtime` reliably. glibc compatibility is added only after that path is stable.

## 13. Non-goals

This decision does not yet define:

- exact physical glibc directory names;
- exact Bundle manifest syntax for `runtime`;
- glibc package/build provenance;
- ABI compatibility guarantees for arbitrary third-party binaries;
- a custom libc;
- a new container format;
- a replacement for Linux namespaces/cgroups.

Those details require separate implementation contracts where necessary.
