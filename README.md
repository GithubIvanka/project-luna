# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has entered architecture-driven backend implementation, integration and hardening.

- Phases **1.1–1.6-HZ** are accepted and consolidated.
- The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); accepted post-1.6 decisions are consolidated there as well.
- The repository/Cargo audit and crate map are established.
- Linux namespace/materialization primitives are implemented in `luna-namespace`.
- Durable system state is implemented in `luna-state` with `redb`.
- Checkpointed update/rollback orchestration exists in `luna-update-manager`.
- `luna-bundle` contains the LBP1 reader/writer implementation for the accepted RFC-0002 format.
- `luna-system-runtime` now supervises real Linux child processes and owns the system-wide UserSession/process lifecycle.
- `luna-app-runtime` now has a real process-launch boundary that can prepare a Linux mount namespace before executing a Bundle entry point.
- The QEMU/OVMF bring-up path now builds a real test SquashFS System Image, early initramfs and separate DATA partition, then hands control to `luna-system-runtime` and an interactive shell.
- `luna-boot.efi` remains a separate bootloader and now selects the kernel/initramfs paths used by the real System Image handoff.

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
  │   └── luna-app-runtime → ApplicationInstance(s)
  └── UserSession B
      └── luna-app-runtime → ApplicationInstance(s)
```

The four physical areas are **EFI / SYSTEM / DATA / SWAP**.

`SYSTEM` contains immutable/versioned Luna System Images and kernels. A System Image is directly a `luna-X.Y.Z.squashfs` SquashFS filesystem image.

`DATA` contains mutable system, user and cache state. The canonical model is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/<user>/
│   ├── home/
│   ├── data/
│   └── config/
└── cache/
```

## Bundles

Applications and installable components use immutable Luna Bundles.

```text
application.lbp
    ↓
Bundle Format v1
```

`.lbp` is the **transport/archive representation** of a Bundle. RFC-0002 — Bundle Format v1 is **Accepted**.

The accepted LBP1 format has a fixed 64-byte little-endian header, fixed 64-byte section entries, TOML manifest, deterministic TAR payload, BLAKE3-256 integrity/content identity, logical mapping declarations, request-only capabilities, and an optional Ed25519 signature section.

Different versions of the same application are independent immutable Bundles and may coexist.

## Runtime and isolation

`luna-system-runtime` is the system-wide runtime/supervisor. `UserSession` is the combined user/session entity.

`luna-app-runtime` manages `ApplicationInstance` lifecycle and prepares an isolated execution environment. Every ApplicationInstance receives its own filesystem/mount namespace. The application sees a normal Linux-compatible logical `/`; physical Luna DATA/SYSTEM paths and mapping tables remain implementation details.

The current Linux launcher prepares the namespace in the child immediately before `exec`. This is a bring-up implementation boundary; production hardening will replace the post-fork setup path with a dedicated child-creation primitive before multi-threaded runtime use.

`luna-security` is the central policy authority. `luna-root-mapping` defines logical mapping semantics. `luna-namespace` contains Linux-specific namespace/materialization primitives. Linux namespaces, cgroups and related kernel mechanisms are enforcement primitives for the Luna model.

## Management boundaries

```text
luna-app-manager
    → install / import / verify / update / remove / migrate Bundles

luna-system-manager
    → system-state model and queries

luna-kernel-manager
    → kernel inventory / compatibility / queries

luna-update-manager
    → mutation coordination / checkpoints / apply / verify / rollback
```

Domain managers retain ownership of their own state; `luna-update-manager` does not become the owner of application, kernel or System Image semantics.

## QEMU bring-up

The reproducible test path is under `boot/luna-boot/tests/ovmf/`.

Build and run the real userspace bring-up with:

```bash
export OVMF_CODE=/path/to/OVMF_CODE.fd
export OVMF_VARS=/path/to/writable/OVMF_VARS.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export BUSYBOX=/path/to/static/x86_64/busybox

boot/luna-boot/tests/ovmf/build-and-run.sh
```

The harness creates an EFI partition, SYSTEM partition and DATA partition, builds a test SquashFS System Image containing `luna-system-runtime`, builds the early initramfs, boots the Linux kernel through `luna-boot.efi`, constructs the logical root and starts the first UserSession with an interactive shell.

This is the first end-to-end development bring-up path. It is deliberately a test image, not a production installer or release image.

## Current crate map

The current userspace workspace contains these architecture-defined crates:

```text
luna-common
luna-fs
luna-root-mapping
luna-namespace
luna-config
luna-security
luna-state
luna-event
luna-bundle
luna-app-manager
luna-system-manager
luna-update-manager
luna-device-manager
luna-kernel-manager
luna-system-runtime
luna-user-session
luna-app-runtime
luna-cli
```

The bootloader is intentionally outside the userspace workspace.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md) for current responsibility boundaries.

## Development rule

`docs/ARCHITECTURE.md` is the primary and current architectural Source of Truth. Historical phase records, ADRs and RFC documents preserve traceability; accepted current semantics are consolidated in the Source of Truth.

Accepted architecture decisions must not be silently changed by implementation work. When code reveals a real architectural conflict, that conflict must be documented and resolved explicitly.

## Current next targets

1. Finish fine-grained security-to-mapping/device authorization and filtered `/dev` population.
2. Connect durable state ownership to `luna-system-runtime` and domain managers.
3. Connect concrete update backends to `luna-system-manager`, `luna-kernel-manager` and `luna-app-manager`.
4. Finish LBP1 conformance and Ed25519 verification/trust binding.
5. Formalize System Image/kernel manifests, compatibility and persistent boot-success state.
6. Add IPC/event transport, resource enforcement, device/volume integration and end-to-end application launch tests.

## License

Apache License 2.0.
