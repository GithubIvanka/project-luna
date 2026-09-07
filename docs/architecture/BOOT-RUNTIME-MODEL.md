# Project Luna — Boot and Runtime Model

**Status:** architecture direction under active design
**Branch:** `develop`
**Date:** 2026-09-07

This document records the current architectural direction discussed for the boundary between `luna-boot`, the Linux kernel, `luna-init`, `luna-system-runtime`, and application runtimes.

## 1. Goal

Luna should not reproduce the classical distribution boot chain merely because Linux traditionally uses it:

```text
UEFI → bootloader → kernel → initramfs → root switch → init → system
```

The target direction is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init (PID 1)
  ↓
managed system environment
  ↓
luna-system-runtime
  ↓
UserSession / application runtimes
```

The goal is to move as much boot preparation as technically appropriate to the pre-kernel/kernel-build side of the boundary and avoid a classic temporary-root → final-root transition.

## 2. luna-boot is not a Linux kernel driver loader

`luna-boot.efi` operates in the UEFI environment. It cannot simply transfer a UEFI driver into Linux and thereby satisfy Linux driver requirements.

The intended mechanism is instead:

```text
luna-boot
  ├── discovers hardware/platform information available through UEFI
  ├── selects compatible System Image + kernel
  ├── validates boot metadata
  └── prepares Linux boot handoff

Luna kernel build
  └── contains the Linux drivers required by the selected platform as built-in
      features where that is appropriate
```

The exact split of responsibilities is still subject to implementation research.

## 3. Initramfs direction

Luna aims to avoid a conventional distro initramfs whose main jobs are:

- loading boot-critical kernel modules;
- discovering the root device;
- constructing a temporary root;
- mounting the final root;
- performing `switch_root` / `pivot_root`-style transition.

The architectural goal is not to preserve these historical layers unnecessarily.

This does **not** yet mean that a byte-level implementation without any early-userspace mechanism has been proven. The exact Linux boot contract must be validated before implementation is changed.

## 4. Root model

The final user-facing Linux filesystem is a logical `/` backed by RAM/runtime storage rather than a persistent disk-backed root filesystem.

The desired conceptual flow is:

```text
System Image (immutable source)
            ↓
   boot/runtime preparation
            ↓
      RAM-backed logical /
            ↓
   luna-system-runtime
```

The architecture explicitly does not require `switch_root` or `pivot_root` as part of the normal boot path. Whether any internal kernel namespace/mount operation is needed to construct the initial mount tree is an implementation detail, not a requirement to expose a classic root-switch model.

## 5. SYSTEM visibility

The physical `SYSTEM` partition is an OS-managed storage source, not the runtime root of `luna-system-runtime`.

`luna-system-runtime` should receive a prepared filesystem/resource view and should not need to know:

- which physical disk contains the image;
- which GPT partition contains SYSTEM;
- which block device path identifies SYSTEM;
- how the System Image was physically located.

Conceptually:

```text
Physical machine
    ↓
luna-init / system host layer
    ↓
prepared resource view
    ↓
luna-system-runtime
```

## 6. luna-init

Current direction:

```text
Linux kernel
    ↓
luna-init
```

`luna-init` is the first userspace process and therefore holds PID 1 in the initial Linux PID namespace.

Its purpose is to be the smallest system host/supervisor needed to bring the rest of Luna online.

It is expected to own knowledge about resources and host boundaries that should not leak into the managed system environment.

The exact long-term PID-1 lifecycle contract (permanent supervisor vs bootstrap handoff) is still open and must be decided explicitly.

## 7. luna-system-runtime as a managed system environment

`luna-system-runtime` is intentionally **not** a KVM/QEMU-style virtual machine.

It is a managed execution environment built from Linux kernel primitives:

```text
                 luna-init
                     │
        ┌────────────┼────────────┐
        │            │            │
   mount namespace  cgroup v2  security policy
        │            │            │
        └────────────┼────────────┘
                     ↓
          luna-system-runtime
```

Its environment is defined by what `luna-init` grants/provides, not by direct knowledge of physical storage.

Namespaces answer primarily **what the environment can see**; cgroup v2 answers primarily **which resources/process groups it can consume and how they are controlled**.

## 8. Common environment model

The same conceptual mechanism should eventually serve both the system runtime and applications.

```text
EnvironmentPlan
    ↓
Authorization
    ↓
Namespace / resource materialization
    ↓
Managed execution environment
```

`luna-system-runtime` and an application differ mainly in policy, mappings, capabilities, lifecycle, and resource limits—not because they require unrelated isolation architectures.

This keeps the previously established application pipeline intact and avoids creating `luna-app-init` or a generic `luna-runtime` layer.

## 9. Linux primitives

Potential primitives include:

- mount namespaces;
- user namespaces where appropriate;
- network namespaces where appropriate;
- IPC/UTS namespaces where appropriate;
- cgroup v2;
- Linux credentials and capabilities;
- seccomp/LSM-based policy where required;
- bind mounts and other controlled filesystem mappings.

No namespace should be enabled merely because it exists. Each namespace must have a concrete architectural purpose.

In particular, a PID namespace is **not** part of the default model merely to make an application process PID 1. The system runtime remains PID 1 at the host/system level; application processes may use ordinary non-1 PIDs.

## 10. Kernel configuration policy

Luna should specialize its Linux kernel through Kconfig rather than maintaining a large custom kernel fork.

The conceptual target is:

```text
required Luna/platform functionality → built-in where boot-critical
unused functionality                 → disabled where safe
```

A kernel `.config` is **not** assumed to be permanently identical across kernel versions. Kernel configuration must be treated as a version-aware build policy that can be adapted to each supported kernel release.

The project may later define a higher-level Luna kernel feature policy and generate/adapt the concrete `.config` for each kernel release.

## 11. Kernel command line direction

Luna should not make a user-visible or mutable kernel command line the primary configuration mechanism.

Potential information channels include:

- Linux boot protocol structures;
- EFI/firmware-provided platform information;
- embedded kernel configuration/boot configuration where appropriate;
- a dedicated Luna boot handoff structure where technically justified.

The exact minimal handoff contract is still open.

## 12. GUI-only normal boot

The normal user path remains graphical:

```text
UEFI
 ↓
luna-boot
 ↓
Linux kernel
 ↓
luna-init
 ↓
luna-system-runtime
 ↓
graphical login
 ↓
Wayland → niri → Noctalia
```

TTY/console is not a normal user interface. Removing unnecessary legacy console/VT functionality from the Luna kernel is desirable where dependencies allow it, while keeping enough diagnostic capability for development/recovery remains a separate design decision.

## 13. Open technical questions

The following items remain implementation/design work and are intentionally **not** silently decided here:

1. exactly which initramfs responsibilities can be moved to `luna-boot`;
2. which Linux boot protocol structures are sufficient for the desired handoff;
3. whether Luna needs a minimal early-userspace image at all;
4. exactly how the initial RAM-backed `/` is constructed without a classic root switch;
5. how the selected SquashFS source is made available to `luna-init` without exposing physical SYSTEM semantics to the managed system runtime;
6. whether `luna-init` remains PID 1 permanently or delegates supervision while remaining the lifecycle owner;
7. the exact namespace/cgroup policy for `luna-system-runtime`;
8. which Kconfig features can be disabled safely for the GUI-only Luna kernel;
9. the final non-cmdline boot configuration/handoff mechanism.

These questions should be resolved through Linux kernel documentation, small experiments, and explicit architecture decisions before major implementation changes.
