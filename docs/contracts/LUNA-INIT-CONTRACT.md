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

## 2. Physical storage vs logical root

Luna has two persistent physical filesystems relevant to normal system operation:

```text
SYSTEM  → immutable system images and kernels
DATA    → mutable user/application/system data
```

The active Linux `/` is neither physical SYSTEM nor physical DATA. It is a RAM-backed runtime filesystem assembled by `luna-init`.

The intended mapping is:

```text
physical SYSTEM ──┐
                  ├── source / hydration ──→ RAM-backed logical /
physical DATA ────┘                         └→ logical /data
```

SYSTEM is an implementation/storage boundary, not a user-visible filesystem.

## 3. Inputs

`luna-init` receives a boot context from the kernel command line and/or an equivalent future handoff mechanism. The context identifies:

- SYSTEM device;
- DATA device;
- selected System Image path;
- selected kernel/image compatibility context;
- boot mode when a special recovery/factory mode was selected.

The selected image must be validated before being used as a source.

## 4. System Image handling

The canonical System Image is:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
```

`luna-init` mounts SYSTEM read-only during bootstrap and mounts the selected SquashFS read-only as an immutable source.

Neither SYSTEM nor the selected System Image becomes the logical `/`.

There is no extra source layer such as `/run/luna-system`, `/run/luna-image`, or another path inside the logical root.

During the root transition the source mounts remain outside the future logical root. `luna-init` hands trusted directory FDs for those sources to `luna-system-runtime`; after the root transition the old initramfs tree is detached, so the sources are not reachable by pathname from the logical root.

## 5. RAM-backed logical root

`luna-init` creates the runtime root as a dedicated tmpfs and makes it the actual Linux root through `pivot_root`.

The initial root must contain at least:

- the directories required by the base Linux userspace;
- boot-critical system executables and their runtime dependencies;
- `/etc` data required for bootstrap;
- `/dev` runtime view;
- `/proc`;
- `/sys`;
- `/run`;
- `/tmp`.

`DATA` is attached directly at `RAM-root/data`, which becomes logical `/data` after the root transition.

The implementation must not copy the entire System Image into RAM merely to simplify bootstrap.

## 6. Initial materialization

The selected System Image is the source for an explicit boot-critical materialization set.

The current implementation keeps this set as an explicit bootstrap list in `luna-init` rather than copying the complete System Image. The complete dependency closure is still an open hardening requirement for production images.

For each resource, `luna-init` validates presence in the mounted immutable source and copies it into the RAM-backed root while preserving ordinary file/symlink semantics through BusyBox `cp -a`.

## 7. Lazy hydration and source lifetime

After the initial boot-critical set is materialized, additional immutable System Image resources may be hydrated lazily.

The long-term design is:

```text
hidden SYSTEM/Image source
          ↓ hydrate
RAM-backed runtime resource
          ↓
active process
```

A materialized resource is independent of the lifetime of the source mount.

The source itself must remain available to the trusted runtime/hydration layer until all still-required resources have either been materialized or otherwise made independently available. `luna-init` therefore does not decide the source retirement point.

Lazy eviction of already materialized RAM data is a separate mechanism. It must not be implemented by blindly unmounting the source or deleting active files; later work must account for open file descriptors, memory mappings, and process dependencies.

## 8. SYSTEM access policy

SYSTEM is read-only for normal system operation and is not exposed as a user filesystem.

The intended authority model is:

```text
ordinary user        → no SYSTEM access
applications         → no SYSTEM access
normal runtime       → controlled read-only source access for hydration
luna-updater         → sole component authorized to modify SYSTEM
```

The updater's write path is a separate privileged update mechanism. Boot-time source mounting remains read-only.

## 9. Runtime pseudo-filesystems

`luna-init` prepares runtime-generated filesystems/paths rather than copying persistent versions from SYSTEM:

```text
/dev
/proc
/sys
/run
/tmp
```

The `/dev` view is controlled and must not become an unrestricted alias of the host device tree.

## 10. Transfer of control

Once the logical root and required runtime filesystems are ready, `luna-init`:

1. preserves trusted FDs for the SYSTEM and selected image sources;
2. unmounts the temporary initramfs `/proc`, `/sys`, and `/dev` mounts;
3. performs `pivot_root` to make the RAM-backed filesystem the actual `/`;
4. detaches the old initramfs tree so its physical source paths are outside the logical root;
5. executes `/sbin/init` directly from the RAM root.

The intended steady-state model is:

```text
PID 1 → luna-system-runtime
```

`luna-init` is a bootstrap component, not a second long-lived system manager.

## 11. Application process model

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

## 12. Failure semantics

A mandatory bootstrap failure must stop normal boot and enter the appropriate recovery/emergency path.

The runtime root must be built transactionally: mounts and temporary resources created before an error are unwound before handing control to a failure path. Complete rollback/recovery cleanup remains a hardening item.

## 13. Current implementation status

The current `develop` implementation provides:

- dedicated RAM-backed logical `/`;
- direct DATA attachment at logical `/data`;
- SYSTEM mounted read-only for bootstrap;
- selected SquashFS mounted read-only as an internal source outside the future logical root;
- explicit bootstrap subset rather than whole-image copy;
- trusted source FD handoff across the root transition;
- runtime-generated `/dev`, `/proc`, `/sys`, `/run` and `/tmp`;
- `pivot_root` into the RAM-backed filesystem;
- final execution of `/sbin/init = luna-system-runtime`.

Remaining implementation work includes:

- manifest-driven boot-critical materialization/dependency closure;
- secure file/tree materialization for all supported object types;
- lazy hydration service/protocol over the trusted source FDs;
- enforcement of the SYSTEM updater-only write authority;
- final `/dev` policy;
- privileged end-to-end boot tests;
- integration of image-retirement checks with runtime materialization state.
