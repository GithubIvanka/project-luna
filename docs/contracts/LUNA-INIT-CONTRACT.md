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

## 2. Inputs

`luna-init` receives a validated `LunaBootHandoffV1` through the kernel integration.

The context identifies:

- SYSTEM partition by GPT disk + partition identity;
- DATA partition by GPT disk + partition identity;
- selected System Image identity and digest;
- running kernel identity and digest;
- `luna-init` identity and digest;
- boot attempt identity;
- boot mode;
- boot-state context.

The canonical userspace channel is:

```text
FD 3 = read-only Luna boot-context
```

FD 3 begins at offset 0 and contains the serialized validated handoff. `luna-init` closes FD 3 after parsing it.

## 3. No initramfs

Project Luna has no initramfs userspace layer in the production boot architecture.

The kernel is built with the complete driver/filesystem dependency closure required to reach SYSTEM and execute the memory-resident `luna-init` ELF directly.

The supported chain is:

```text
Linux kernel
    ↓
validated Luna boot context
    ↓
memory-resident luna-init ELF
    ↓
luna-init PID 1
```

A cpio/gzip `luna-initramfs.img`, an initramfs `/init`, `switch_root` and `pivot_root` are not part of the target architecture.

## 4. Responsibility

`luna-init` owns early userspace bootstrap and construction of the initial System Environment. It may reason about physical storage, block devices, hardware and firmware information exposed by Linux.

It does not own UserSession or application lifecycle.

## 5. SYSTEM and System Image

The canonical storage layout is:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
SYSTEM/images/luna-X.Y.Z.init
SYSTEM/kernels/<kernel-id>/bzImage
```

`luna-init` resolves the SYSTEM identity from the handoff, mounts SYSTEM read-only, verifies that the selected image and manifest match the handoff identity, and uses the SquashFS as an immutable source.

The `.init` artifact is the exact ELF already loaded by `luna-boot` into reserved memory; `luna-init` does not load or select another initial userspace artifact.

## 6. RAM-backed logical root

The logical system environment is RAM-backed. Boot-critical files are materialized according to the System Image bootstrap/materialization contract rather than copying the entire System Image merely for convenience.

The physical SYSTEM partition and the selected SquashFS remain internal sources and are not exposed as ordinary user filesystem paths.

## 7. DATA

DATA is persistent storage, not the logical root. `luna-init` provides trusted physical access to the DATA identity received in the handoff; higher policy layers decide which DATA resources become visible in the System Environment.

## 8. Runtime filesystems

`luna-init` establishes the prerequisites for:

```text
/dev
/proc
/sys
/run
/tmp
```

These are runtime facilities and are not persistent copies of SYSTEM.

## 9. Starting `luna-system-runtime`

Once the minimal System Environment is ready:

```text
luna-init (PID 1)
        ↓
luna-system-runtime
```

`luna-init` remains PID 1, reaps children and owns system-wide lifecycle obligations.

## 10. Failure semantics

`luna-init` fails closed on:

- missing or malformed FD 3;
- invalid handoff ABI;
- inconsistent SYSTEM/DATA identity;
- mismatched System Image or manifest identity;
- unavailable required storage;
- invalid required bootstrap resources;
- failure to start `luna-system-runtime`.

It must not fall back to unrelated paths or legacy command-line selectors when the structured boot context is invalid.

## 11. Implementation rule

The old transitional implementation must not be extended. Any remaining code that assumes an initramfs, BusyBox bootstrap root, `pivot_root`, `switch_root`, or a second `/sbin/init` belongs to the obsolete implementation and should be removed as the direct-init path is completed.
