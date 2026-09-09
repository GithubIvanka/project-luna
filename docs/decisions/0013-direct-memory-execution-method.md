# ADR-0013 — Direct memory-resident `luna-init` execution method

**Status:** Accepted  
**Date:** 2026-09-09  
**Scope:** Luna kernel implementation of the direct initial-userspace path

## Decision

The Luna kernel shall launch the bootloader-selected `luna-init` directly from the memory object prepared by `luna-boot.efi`.

The canonical execution path is:

```text
UEFI
  ↓
luna-boot.efi
  │
  ├─ Linux kernel image
  ├─ memory-resident luna-init ELF
  └─ LunaBootHandoffV1
        ↓
Linux kernel
  ↓
Luna kernel direct-userspace execution path
  ↓
Linux ELF/binfmt loading machinery
  ↓
luna-init (PID 1)
```

## 1. No filesystem bootstrap layer

The direct-init implementation MUST NOT implement the Linux userspace bootstrap sequence through:

- an initramfs archive;
- a temporary userspace root;
- a temporary `/init`;
- `/luna-init` as a staged filesystem pathname;
- `switch_root`;
- `pivot_root`;
- a second `/sbin/init` lookup;
- BusyBox or another bootstrap utility layer.

`luna-init` is the first Luna userspace process and becomes PID 1 directly after the kernel finishes its own initialization.

## 2. Memory-resident executable object

`luna-boot.efi` supplies the exact `luna-init` ELF byte range in boot-reserved physical memory and identifies it through the `LUNA_INIT_IMAGE` handoff record.

The kernel-side Luna implementation may create a kernel-internal memory-backed executable object solely to reuse the existing Linux ELF/binfmt machinery.

Such an object:

- is not a userspace pathname;
- is not mounted into any filesystem namespace;
- is not a root filesystem;
- is not visible as ordinary persistent storage;
- exists only for the initial executable loading lifetime;
- is released according to the kernel execution object's lifetime rules after successful ELF loading.

The existence of this internal object MUST NOT introduce a root transition or another userspace layer.

## 3. Implementation language boundary

Luna-specific policy and execution logic belong in the Rust kernel integration:

```text
kernel/rust/luna_boot.rs
kernel/rust/luna_exec.rs
```

The upstream Linux kernel remains largely unchanged. Where an internal Linux exec subsystem API cannot be reached from Rust directly, only the minimum kernel-internal adapter necessary to expose that existing functionality may be added to the patched Linux tree.

Such an adapter is plumbing for the Linux execution subsystem and MUST NOT contain Luna boot policy, storage discovery, root construction, fallback logic or userspace bootstrap behavior.

## 4. ELF loading

The kernel MUST reuse Linux's existing ELF/binfmt loader for the final userspace virtual-memory construction whenever possible.

Luna does not implement a second userspace ELF loader merely to avoid using Linux's mature execution machinery.

The Luna-specific path is responsible for:

1. validating `LunaBootHandoffV1`;
2. validating the `LUNA_INIT_IMAGE` range and integrity;
3. creating the kernel-internal executable object from the exact memory-resident bytes;
4. preparing the initial process context;
5. installing the validated handoff as FD 3;
6. entering the existing Linux ELF/binfmt execution machinery;
7. treating failure as a boot failure rather than selecting an unrelated init path.

## 5. Initial process semantics

The execution happens in the kernel's existing initial-userspace task (`kernel_init` path).

On success:

```text
PID 1 = luna-init
```

The kernel MUST NOT subsequently execute another init path.

`luna-system-runtime` is started later by `luna-init` as a child and is never PID 1.

## 6. Handoff delivery

Before transferring execution to `luna-init`, the kernel installs the exact validated `LunaBootHandoffV1` byte sequence as:

```text
FD 3
```

FD 3 is read-only, positioned at offset zero and intentionally inherited by the initial `luna-init` process. `luna-init` consumes and closes the descriptor before creating normal child processes.

This follows ADR-0012 and does not depend on a root filesystem or pathname.

## 7. Memory lifetime

The physical `luna-init` bytes are boot-reserved. The kernel retains them until the execution/loading path no longer requires them.

The kernel MUST NOT treat the boot-reserved range as ordinary free memory prematurely.

The handoff object and executable object have independent lifetimes:

```text
boot-reserved ELF bytes
        ↓
initial executable object
        ↓
ELF load into userspace VM
        ↓
original boot bytes may be released

boot handoff
        ↓
FD 3
        ↓
luna-init consumes context
        ↓
FD 3 closes
```

## 8. Failure semantics

Any failure in the direct initial-userspace path is fatal for the current boot attempt:

```text
invalid handoff
    ↓
invalid init image
    ↓
execution setup failure
    ↓
ELF loader failure
    ↓
boot failure
```

The kernel MUST NOT fall back to `/init`, `/sbin/init`, `/bin/sh`, an initramfs or a command-line-selected alternate userspace.

Fallback to another System Image/kernel remains a `luna-boot` / Boot State responsibility.

## 9. Explicitly rejected implementation

The following implementation is explicitly rejected:

```text
memory-resident luna-init
        ↓
copy to /luna-init
        ↓
kernel_execve("/luna-init")
```

Although it can reuse Linux's normal ELF loader, it reintroduces a filesystem pathname dependency and makes the kernel execution step look like ordinary rootfs-based Linux boot. It is not the canonical Luna execution model.

## 10. Relationship to existing contracts

This ADR refines and implements, without changing, the previously accepted decisions in:

- `docs/decisions/0009-direct-initial-userspace.md`;
- `docs/decisions/0010-versioned-luna-init-artifact.md`;
- `docs/decisions/0011-canonical-pid1-and-direct-boot-chain.md`;
- `docs/decisions/0012-luna-init-boot-context-fd.md`.

The semantic contract remains:

```text
luna-boot
    ↓
prepared memory + handoff
    ↓
Linux kernel
    ↓
Luna direct execution hook
    ↓
luna-init PID 1
```

No new root layer is introduced.
