# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is now in **Phase 2 runtime/boot integration and hardening**.

- Phases **1.1–1.6-HZ** are accepted and consolidated.
- The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); accepted post-1.6 decisions are consolidated there as well.
- Linux namespace/materialization primitives are implemented in `luna-namespace`.
- Durable system state is implemented in `luna-state` with `redb`.
- Checkpointed update/rollback orchestration exists in `luna-update-manager`.
- `luna-bundle` contains the LBP1 reader/writer implementation for the accepted RFC-0002 format.
- `luna-system-runtime` supervises real Linux child processes and owns the system-wide UserSession/process lifecycle.
- `luna-app-runtime` owns `ApplicationInstance` lifecycle and execution setup.
- `RuntimeKind`/`RuntimeSpec` are typed execution-environment values used by application runtime; there is no separate generic runtime subsystem/crate.
- Native early userspace is provided by the standalone musl `luna-init` binary and hands off to `luna-system-runtime` after `switch_root`.
- The QEMU/OVMF bring-up path builds a real SquashFS System Image, early initramfs and separate DATA partition.
- A reproducible x86_64 UEFI/GPT PC development image is produced by `tools/build-pc-image.sh` with native Luna initramfs and the graphical desktop payload.
- The development PC image packages the native niri/Noctalia graphical stack, login, Ghostty, fish, Yazi, audio, network, Bluetooth and removable-media service payloads; final hardware seat/input/GPU validation remains.

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Core architectural model

```text
UEFI
  ↓
luna-boot.efi
  ↓
SYSTEM
  ├── versioned System Images (direct SquashFS)
  └── versioned kernels
  ↓
early userspace / luna-init
  ↓
logical Linux-compatible root + DATA
  ↓
luna-system-runtime
  ├── UserSession A
  │   └── luna-app-runtime
  │       └── ApplicationInstance(s)
  └── UserSession B
      └── luna-app-runtime
          └── ApplicationInstance(s)
```

The four physical areas are **EFI / SYSTEM / DATA / SWAP**.

`SYSTEM` contains immutable/versioned Luna System Images and kernels. A System Image is directly a `luna-X.Y.Z.squashfs` SquashFS filesystem image.

`DATA` contains mutable system, user and cache state.

## Application execution flow

The hierarchy above is ownership, not the security pipeline. A normal application launch conceptually proceeds as:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
    ↓
Bundle/resource declarations
    ↓
Mapping plan
    ↓
Security authorization
    ↓
luna-namespace materialization
    ↓
process execution
    ↓
luna-system-runtime supervision
```

`RuntimeKind` is only a typed property of the execution environment used by the application runtime, for example:

```text
Luna   → native Luna userspace / musl
Glibc  → approved glibc compatibility runtime
Bundle → Bundle-private runtime where permitted
```

It is not a separate daemon, manager or layer in the runtime hierarchy.

## Bundles

Applications and installable components use immutable Luna Bundles.

```text
application.lbp
    ↓
Bundle Format v1
```

`.lbp` is the **transport/archive representation** of a Bundle. RFC-0002 — Bundle Format v1 is **Accepted**.

Different versions of the same application are independent immutable Bundles and may coexist.

## Boot and user experience

Normal boot is graphical and quiet:

```text
Power
 ↓
UEFI
 ↓
luna-boot.efi
 ↓
GUI boot splash
 ↓
Linux kernel
 ↓
luna-init
 ↓
System Image + DATA
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
GUI login
 ↓
authentication
 ↓
Active UserSession
 ↓
Wayland
 ↓
niri
 ↓
Noctalia Shell
```

There is no normal TTY login, console-shell fallback or `luna-session` component.

Pressing `B` opens the exceptional text Boot Menu. The accepted action order is:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

Verbose Boot suppresses the graphical splash and enables full boot diagnostics for that boot. Recovery, Factory and external boot are separate boot modes; their concrete backends must never be emulated by a TTY fallback.

## PC development build

The repository contains a reproducible x86_64 PC image builder:

```bash
tools/build-pc-image.sh
```

It creates `dist/luna-pc.img` with EFI, SYSTEM and DATA partitions. SYSTEM contains the versioned SquashFS System Image, its manifest, initramfs and kernel. DATA contains persistent Luna state.

See [`docs/development/PC-BUILD.md`](docs/development/PC-BUILD.md).

## Current crate map

The current userspace workspace contains architecture-defined crates only; `luna-init` is the deliberate standalone early-userspace exception. There is no generic `luna-runtime` crate.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md).

## Development rule

`docs/ARCHITECTURE.md` is the primary and current architectural Source of Truth. Historical phase records, ADRs and RFC documents preserve traceability; accepted current semantics are consolidated in the Source of Truth.

Accepted architecture decisions must not be silently changed by implementation work. When code reveals a real architectural conflict, that conflict must be documented and resolved explicitly.
