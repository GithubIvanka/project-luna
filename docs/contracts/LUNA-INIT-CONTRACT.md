# Luna Init Contract

**Status:** accepted architecture / implementation contract in progress  
**Scope:** `luna-init` early userspace bootstrap

## 1. Position in boot chain

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init
  ↓
RAM-backed logical /
  ↓
luna-system-runtime (PID 1)
```

`luna-init` is the first Luna userspace bootstrap boundary after the kernel.

## 2. Inputs

`luna-init` receives a boot context from the kernel command line and/or an equivalent future handoff mechanism. The context identifies:

- SYSTEM device;
- DATA device;
- selected System Image path;
- selected kernel/image compatibility context;
- boot mode when a special recovery/factory mode was selected.

The selected image must be validated before being used as a source.

## 3. System Image handling

The canonical System Image is:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
```

`luna-init` mounts SYSTEM read-only and opens the selected SquashFS as an **internal immutable source**.

It does not make that SquashFS the final Linux root and does not use classic `switch_root` to turn it into `/`.

The SYSTEM filesystem and selected image remain mounted as internal runtime sources after bootstrap. Their mount points live inside the runtime `/run` tree so they survive the final `chroot`. `luna-init` transfers responsibility for their lifetime to `luna-system-runtime` and the future hydration layer; it must not eagerly unmount them merely because initial materialization has completed.

## 4. Runtime root

The final logical `/` is backed by a dedicated tmpfs mounted on the root staging point before DATA is attached.

The initial root must contain at least:

- the directories required by the base Linux userspace;
- boot-critical system executables and their runtime dependencies;
- `/etc` data required for bootstrap;
- `/dev` runtime view;
- `/proc`;
- `/sys`;
- `/run`;
- `/tmp`.

Physical SYSTEM and DATA paths are not part of the application-facing filesystem contract.

## 5. Initial materialization

The selected System Image is the source for an explicit boot-critical materialization set.

The current implementation keeps this set as an explicit bootstrap list in `luna-init` rather than copying the complete System Image. The set includes the native system supervisor, its bootstrap configuration, and the graphical-session entry points required by the current PC development image.

For each resource, `luna-init` validates presence in the mounted immutable source and copies it into the RAM-backed root while preserving normal file/symlink semantics through BusyBox `cp -a`. The complete dependency closure is still an open hardening requirement for production images.

The implementation must not copy the entire System Image into RAM merely to simplify bootstrap.

## 6. Lazy hydration

After the initial boot-critical set is materialized, additional immutable System Image resources may be hydrated lazily.

The long-term design must satisfy:

```text
System Image source
      ↓ hydrate
RAM-backed runtime resource
      ↓
active process
```

A resource that has been materialized into the active runtime must remain usable independently of the lifetime of the original System Image mount.

The original source is therefore kept available while runtime state may still require hydration from it. Source unmounting is a lifecycle decision made by the runtime/hydration layer, not by `luna-init` immediately after bootstrap.

Lazy hydration must not expose SYSTEM paths to applications.

The exact request/lookup protocol for lazy hydration is a separate implementation task, but it must use the same trust and path-validation rules as the boot path.

Lazy eviction is a separate concern: it may only reclaim RAM-backed resources after the runtime has a complete strategy for open file descriptors, mappings, process dependencies, and rehydration. It is not implied by source unmounting.

## 7. Runtime pseudo-filesystems

`luna-init` prepares runtime-generated filesystems/paths rather than copying persistent versions from SYSTEM:

```text
/dev
/proc
/sys
/run
/tmp
```

The `/dev` view is controlled and must not become an unrestricted alias of the host device tree.

## 8. DATA

DATA is mounted/attached independently from SYSTEM and remains mutable persistent storage.

Its physical hierarchy is hidden behind the logical runtime mapping model.

`luna-init` prepares only the bootstrap-level DATA access needed before `luna-system-runtime` takes over. Normal application DATA mapping belongs to higher runtime/security layers.

## 9. Transfer of control

Once the logical root and required runtime filesystems are ready, `luna-init` validates `/sbin/init`, leaves the SYSTEM and image source mounts attached, and executes BusyBox `chroot` into the RAM-backed root. The chroot then executes `/sbin/init`, which is the `luna-system-runtime` binary in the materialized root.

The intended steady-state model is:

```text
PID 1 → luna-system-runtime
       │
       └── owns source/hydration lifecycle
```

`luna-init` is a bootstrap component, not a second long-lived system manager.

## 10. Application process model

`luna-init` does not create `luna-app-init` and does not create an application PID-1 supervisor.

Application execution remains:

```text
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationPlan
  ↓
Authorization
  ↓
ApplicationLaunchContext
  ↓
luna-namespace
  ↓
ApplicationInstance
  ↓
application process
```

A PID namespace is not mandatory for ApplicationInstance isolation. By default applications remain ordinary processes in the system PID namespace and receive normal non-1 PIDs.

## 11. Failure semantics

A mandatory bootstrap failure must stop normal boot and enter the appropriate recovery/emergency path.

The runtime root must be built transactionally: mounts and temporary resources created before an error are unwound before handing control to a failure path. After successful handoff, source mount lifetime belongs to the runtime/hydration layer rather than `luna-init`.

## 12. Current implementation status

The historical `switch_root` implementation has been replaced on `develop` by the first RAM-root bootstrap implementation.

Current implementation provides:

- dedicated tmpfs root;
- explicit bootstrap subset rather than whole-image copy;
- immutable SquashFS kept as an internal source;
- independent DATA attachment under logical `/data`;
- runtime-generated `/dev`, `/proc`, `/sys`, `/run` and `/tmp`;
- SYSTEM and selected image mounted inside the runtime root so they survive `chroot`;
- final `chroot` to the RAM-backed root;
- preservation of `luna-system-runtime` as PID 1.

Remaining implementation work includes:

- manifest-driven boot-critical materialization/dependency closure;
- secure file/tree materialization for all supported object types;
- lazy hydration service/protocol and source lifetime manager;
- final `/dev` policy;
- privileged end-to-end boot tests;
- integration of image-retirement checks with runtime materialization state;
- lazy eviction with complete FD/mmap/process dependency tracking.
