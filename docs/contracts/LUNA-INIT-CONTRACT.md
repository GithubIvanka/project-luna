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

## 4. Runtime root

The final logical `/` is backed by RAM, normally by a dedicated tmpfs inside the Luna runtime mount namespace.

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

The implementation must not copy the entire System Image into RAM merely to simplify bootstrap.

The boot-critical set must be deterministic for a given System Image manifest/runtime profile and must include the complete dependency closure required to start `luna-system-runtime` and the initial required system path.

Materialization must preserve the semantics required by the boot-critical files, including regular-file contents, executable mode and required ownership/permissions. Unsafe or unsupported filesystem object types must fail closed rather than being silently converted.

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

Lazy hydration must not expose SYSTEM paths to applications.

The exact request/lookup protocol for lazy hydration is a separate implementation task, but it must use the same trust and path-validation rules as the boot path.

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

Once the logical root and required runtime filesystems are ready, `luna-init` starts `luna-system-runtime` as the normal userspace supervisor.

The intended steady-state model is:

```text
PID 1 → luna-system-runtime
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

The runtime root must be built transactionally: mounts and temporary resources created before an error are unwound before handing control to a failure path.

## 12. Current implementation status

The repository currently contains a historical `switch_root` prototype in `components/luna-init/src/main.rs`. That code is explicitly not the final architecture described by this contract.

Remaining implementation work includes:

- boot-critical materialization manifest/dependency closure;
- RAM-root constructor;
- secure file/tree materialization from the internal System Image source;
- lazy hydration service/protocol;
- final `/dev` policy;
- privileged end-to-end boot tests;
- removal of the historical `switch_root` path after the replacement is integrated into the PC/QEMU image builder.
