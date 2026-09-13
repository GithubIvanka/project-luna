# Luna Init Contract

**Status:** accepted architecture / direct-init implementation in progress  
**Scope:** `luna-init` as PID 1

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

`luna-init` is the first Luna userspace process and remains PID 1 for the normal system lifetime.

## 2. Independent core component

`luna-init` is a boot/runtime core component with an independent lifecycle from the selected System Image.

Canonical storage:

```text
SYSTEM/cores/
├── luna-X.Y.Z.init
├── luna-X.Y.Z.toml
└── ...
```

The `.toml` file adjacent to a `.init` artifact is the manifest for that init core.

A System Image must never own the lifetime of `luna-init`. The selected `.init` artifact may be loaded into RAM before `ExitBootServices` and remains valid after the source file is no longer accessed.

## 3. Inputs

`luna-init` receives a validated `LunaBootHandoffV1` through the kernel integration.

The context identifies:

- SYSTEM partition by GPT disk + partition identity;
- DATA partition by GPT disk + partition identity;
- selected System Image identity and digest;
- running kernel identity and digest;
- selected `luna-init` identity and digest;
- boot attempt identity;
- boot mode;
- boot-state context.

The canonical userspace channel is:

```text
FD 3 = read-only Luna boot-context
```

FD 3 begins at offset 0 and contains the serialized validated handoff. `luna-init` closes FD 3 after parsing it.

## 4. Direct PID1 execution and dependency closure

`luna-init` is a direct initial userspace executable. Its own execution path must not depend on a runtime ELF interpreter or on dynamically loaded userspace libraries that have to be discovered from the System Image before PID 1 can start.

The canonical build target is:

```text
x86_64-unknown-linux-musl
```

Release builds use a static CRT configuration and `panic = abort`. The resulting artifact is required to be a self-contained ELF64 executable for x86_64 with no requested program interpreter and no dynamic ELF section.

The repository helper:

```text
tools/check-luna-init-static.sh
```

validates these properties against the release artifact.

This is a prerequisite for the later System Image dependency closure: dependency resolution for the System Environment starts only after the direct PID1 executable itself is known to be independently executable.

## 5. No initramfs

Project Luna has no initramfs userspace layer in the production boot architecture.

The kernel is built with the complete driver/filesystem dependency closure required to reach SYSTEM and execute the memory-resident `luna-init` directly.

The supported chain is:

```text
Linux kernel
    ↓
validated Luna boot context
    ↓
memory-resident luna-init
    ↓
luna-init PID 1
```

A cpio/gzip initramfs, an initramfs `/init`, `switch_root` and `pivot_root` are not part of the target architecture.

## 6. Responsibility

`luna-init` owns early userspace bootstrap and construction of the initial System Environment. It may reason about physical storage, block devices, hardware and firmware information exposed by Linux.

It does not own UserSession or application lifecycle.

## 7. SYSTEM and System Image

The canonical storage layout is:

```text
SYSTEM/
├── cores/
│   ├── luna-X.Y.Z.init
│   ├── luna-X.Y.Z.toml
│   └── ...
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
└── kernels/
    └── <kernel-id>/
        └── bzImage
```

`luna-boot` resolves the SYSTEM identity from the handoff, validates access to SYSTEM, verifies that the selected image and manifest match the handoff identity, and uses the selected SquashFS as an immutable source.

The `.init` artifact is an independent core artifact. Its manifest defines the kernels compatible with that init core. The System Image manifest separately defines which init core versions it accepts.

The resulting boot relation is:

```text
System Image
    ↓ compatible init
luna-init
    ↓ compatible kernel
Kernel
```

The `.init` file name is intentionally independent of the internal executable representation. The current direct Linux execution path uses an ELF64 binary payload, while `.init` is the Luna artifact name and is not a user-facing file-format name.

## 8. Init compatibility manifest

For an init core `luna-X.Y.Z.init`, the adjacent manifest `luna-X.Y.Z.toml` describes that init artifact.

The compatibility declaration belongs to the init manifest:

```toml
[init]
name = "luna-init"
version = "2.1.0"

[architecture]
arch = "x86_64"

[kernels]
compatible = ["K1", "K2"]
```

The init manifest must not define System Image compatibility. That relationship belongs to the System Image manifest.

## 9. RAM-backed logical root

The logical system environment is RAM-backed. Boot-critical files are materialized according to the System Image bootstrap/materialization contract rather than copying the entire System Image merely for convenience.

The physical SYSTEM partition and the selected SquashFS remain internal sources and are not exposed as ordinary user filesystem paths.

## 10. DATA

DATA is persistent storage, not the logical root. `luna-init` provides trusted physical access to the DATA identity received in the handoff; higher policy layers decide which DATA resources become visible in the System Environment.

## 11. Runtime filesystems

`luna-init` establishes the prerequisites for:

```text
/dev
/proc
/sys
/run
/tmp
```

These are runtime facilities and are not persistent copies of SYSTEM.

## 12. Starting `luna-system-runtime`

Once the minimal System Environment is ready:

```text
luna-init (PID 1)
        ↓
luna-system-runtime
```

`luna-init` remains PID 1, reaps children and owns system-wide lifecycle obligations.

## 13. Failure semantics

`luna-init` fails closed on:

- missing or malformed FD 3;
- invalid handoff ABI;
- inconsistent SYSTEM/DATA identity;
- mismatched System Image or manifest identity;
- mismatched `luna-init` identity;
- unavailable required storage;
- invalid required bootstrap resources;
- failure to start `luna-system-runtime`.

It must not fall back to unrelated paths or legacy command-line selectors when the structured boot context is invalid.

## 14. Lifecycle independence

The following are independently versioned and retained:

```text
System Image
Kernel
luna-init core
```

A boot target is a compatibility relation among these three artifacts rather than ownership of one artifact by another.

Retention is evaluated independently for each artifact, subject to the requirement that at least one valid compatible boot chain remains available.

## 15. Implementation rule

The old transitional implementation must not be extended. Any remaining code that assumes an initramfs, BusyBox bootstrap root, `pivot_root`, `switch_root`, a second `/sbin/init`, or System Image ownership of `luna-init` belongs to the obsolete implementation and should be removed as the direct-init path is completed.
