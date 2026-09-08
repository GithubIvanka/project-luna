# Luna Init Contract

**Status:** accepted architecture / implementation contract in progress  
**Date:** 2026-09-08  
**Scope:** `luna-init` as the first and system-wide userspace supervisor boundary

## 1. Position in boot chain

```text
UEFI
  ↓
luna-boot.efi
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
  ↓
System Environment
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
```

`luna-init` is the first Luna userspace process after the kernel and remains PID 1 for normal system lifetime.

## 2. Responsibility

`luna-init` is the low-level Luna system supervisor/bootstrap boundary. It knows the real machine and owns the construction of the initial System Environment.

It may know:

- physical disks and partitions;
- Linux block devices;
- device discovery state;
- CPU and memory information;
- firmware/platform information exposed to userspace;
- SYSTEM and DATA storage identities;
- selected System Image and kernel context;
- boot mode and boot-attempt state;
- early resource and recovery state.

`luna-init` must not become the owner of UserSession/application lifecycle. Those responsibilities belong above it.

## 3. Inputs

`luna-init` receives a validated `LunaBootHandoffV1` through the Luna kernel integration.

The handoff identifies:

- SYSTEM partition by GPT disk + partition identity;
- DATA partition by GPT disk + partition identity;
- selected System Image identity and digest;
- running kernel identity;
- boot attempt identity;
- boot mode;
- required boot-state context.

Production boot does not require `luna.system_device`, `luna.data_device` or `luna.system_image` command-line parsing.

## 4. No initramfs architecture

Luna does not introduce a separate initramfs userspace environment between the kernel and `luna-init`.

The kernel is built with the complete driver/filesystem dependency closure required to reach the supported boot storage and directly execute `luna-init`.

Therefore the boot path is:

```text
Linux kernel
    ↓
luna-init
```

not:

```text
Linux kernel
    ↓
initramfs
    ↓
luna-init
```

## 5. System Image handling

The canonical System Image remains:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
```

`luna-init` resolves the SYSTEM identity received in the handoff to the actual Linux block device, mounts SYSTEM read-only, validates the selected image/manifest identity and exposes the SquashFS only as an internal immutable source.

The System Image is never treated as the final physical `/`.

## 6. RAM-backed logical root

The active logical root is RAM-backed. It is constructed directly as the runtime system environment.

Conceptually:

```text
System Image immutable source
        + approved DATA mappings
        + runtime pseudo-filesystems
        ↓
RAM-backed logical /
```

The whole System Image must not be copied merely for convenience.

Boot-critical content is materialized according to the image's boot materialization contract. Additional immutable content can be hydrated later.

## 7. Root transition

The target architecture contains **no** classic `switch_root` or `pivot_root` stage and no second `/sbin/init`.

`luna-init` starts as PID 1 and constructs the system environment in the kernel's initial root context. The implementation must arrange the logical root and mount topology so that `luna-system-runtime` is started as a child of the same PID 1 without a root handoff to another init filesystem.

Any temporary internal mounts/resources used solely while constructing the environment are implementation details and must not create a user-visible filesystem layer.

## 8. DATA

DATA is a persistent physical storage area and is not itself the logical `/`.

The exact logical mapping of DATA resources is owned by Luna's root-mapping and system policy layers. `luna-init` provides the trusted physical access needed by those layers and does not expose raw DATA paths to applications.

## 9. SYSTEM source boundary

The selected System Image remains an internal trusted source. Ordinary users and applications do not receive physical SYSTEM paths.

`luna-init` may establish the initial trusted source access required by `luna-system-runtime` for future hydration/materialization. The exact source access mechanism is part of the namespace/materialization implementation and must remain outside the application filesystem contract.

## 10. Runtime filesystems

`luna-init` establishes the initial kernel/runtime prerequisites for:

```text
/dev
/proc
/sys
/run
/tmp
```

These are runtime state, not persistent copies of SYSTEM.

The `/dev` view must remain subject to Luna device/security policy and must not become unrestricted host device exposure.

## 11. Hardware knowledge

Unlike `luna-system-runtime`, `luna-init` is allowed to reason directly about the physical machine.

The division is:

```text
luna-init
    knows the real machine

luna-system-runtime
    knows the System Environment it was given
```

This prevents higher runtime layers from depending on physical disk paths or firmware-specific discovery mechanics.

## 12. Starting system-runtime

Once the minimal System Environment exists and the prerequisites for normal userspace are satisfied:

```text
luna-init (PID 1)
        ↓
exec/create child execution context
        ↓
luna-system-runtime
```

`luna-init` remains responsible for PID-1 semantics, child reaping and system-wide lifecycle obligations unless a separate future architecture decision changes this.

## 13. Application/runtime boundary

`luna-init` does not launch applications directly.

The accepted hierarchy remains:

```text
luna-init
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

There is no `luna-app-init` component.

## 14. Kernel module model

Boot-critical drivers are built into the kernel. Optional post-boot drivers may be stored as loadable modules in the version-matched kernel artifact under:

```text
SYSTEM/kernels/<kernel-id>/modules/
```

`luna-init` must not require an initramfs just to discover or load the drivers necessary to mount SYSTEM or start itself.

## 15. Failure semantics

Failure before `luna-system-runtime` is operational is a system boot failure and must be classified for the boot/recovery policy.

`luna-init` must fail closed on invalid Handoff, missing mandatory resources, incompatible image identity or unavailable required storage.

It must not silently fall back to an unrelated image/device discovered only through a legacy path-based mechanism.

## 16. Implementation direction

The current `develop` implementation contains transitional RAM-root logic. That code is not the final ABI implementation because it still contains initramfs-era assumptions, including `pivot_root`, BusyBox bootstrap helpers and a second `/sbin/init` execution model.

The implementation target is now:

```text
luna-boot
  ↓ LunaBootHandoffV1
Luna kernel
  ↓ direct initial-userspace launch
luna-init PID 1
  ↓ System Environment
luna-system-runtime
```

The old transitional path must be removed rather than extended.
