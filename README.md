# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system project focused on a small immutable system foundation, predictable architecture, self-contained applications, and a clean user-facing filesystem model.

## Current status

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is now in **Phase 2 runtime/boot integration and hardening**.

- Phases **1.1–1.6-HZ** are accepted and consolidated.
- The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); accepted post-1.6 decisions are consolidated there as well.
- The repository/Cargo audit and crate map are established.
- Linux namespace/materialization primitives are implemented in `luna-namespace`.
- Durable system state is implemented in `luna-state` with `redb`.
- Checkpointed update/rollback orchestration exists in `luna-update-manager`.
- `luna-bundle` contains the LBP1 reader/writer implementation for the accepted RFC-0002 format.
- `luna-system-runtime` supervises real Linux child processes and owns the system-wide UserSession/process lifecycle.
- `luna-app-runtime` has a real process-launch boundary and typed runtime selection (`luna`, `glibc`, `bundle`).
- Runtime selection is bound to mapping and security before namespace materialization.
- The QEMU/OVMF bring-up path builds a real SquashFS System Image, early initramfs and separate DATA partition.
- A reproducible x86_64 UEFI/GPT PC development image is now produced by `tools/build-pc-image.sh`.
- CI builds and uploads the PC development image as `luna-pc-x86_64`.
- The final production graphical payload is not yet packaged; the development image falls back to a usable shell.

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

`RuntimeKind` is now a typed shared contract:

```text
luna   → native Luna userspace / musl
 glibc → approved compatibility runtime
a bundle → Bundle-private runtime where permitted
```

`luna-app-runtime` manages `ApplicationInstance` lifecycle and prepares an isolated execution environment. Every ApplicationInstance receives its own filesystem/mount namespace. The application sees a normal Linux-compatible logical `/`; physical Luna DATA/SYSTEM paths and mapping tables remain implementation details.

Runtime-aware launch ordering is:

```text
RuntimeSpec
   ↓
mapping/runtime validation
   ↓
Security authorization
   ↓
namespace materialization
   ↓
process execution
```

`luna-security` is the central policy authority. Runtime use is represented as `Resource::Runtime(kind)` with `Permission::Use`; selecting a runtime therefore never grants access by itself.

The current Linux application launcher still uses the development `pre_exec` child setup path. Production hardening will replace that post-fork mechanism with a dedicated child-creation primitive.

## Management boundaries

```text
luna-app-manager
    → install / import / verify / update / remove / migrate Bundles

luna-system-manager
    → durable system-state model and queries

luna-kernel-manager
    → kernel inventory / compatibility / queries

luna-update-manager
    → mutation coordination / checkpoints / apply / verify / rollback
```

Domain managers retain ownership of their own state; `luna-update-manager` does not become the owner of application, kernel or System Image semantics.

## PC development build

The repository now contains a reproducible x86_64 PC image builder:

```bash
tools/build-pc-image.sh
```

It creates:

```text
dist/luna-pc.img
```

with GPT partitions:

```text
EFI     128 MiB
SYSTEM  384 MiB
DATA    512 MiB
```

The build uses the native musl target for `luna-system-runtime`, packages the versioned SquashFS System Image and initramfs into SYSTEM, and creates a persistent DATA partition for Luna state.

The build script automatically discovers a host Linux kernel and static BusyBox when available. Explicit paths can be supplied with `LUNA_TEST_KERNEL` and `BUSYBOX`.

The image can be written to a dedicated PC disk with the guarded installer:

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/<whole-disk> --yes
```

See [`docs/development/PC-BUILD.md`](docs/development/PC-BUILD.md) for the complete procedure and limitations.

## QEMU bring-up

The reproducible QEMU/OVMF test path remains under `boot/luna-boot/tests/ovmf/`.

Build and run it with:

```bash
export OVMF_CODE=/path/to/OVMF_CODE.fd
export OVMF_VARS=/path/to/writable/OVMF_VARS.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export BUSYBOX=/path/to/static/x86_64/busybox

boot/luna-boot/tests/ovmf/build-and-run.sh
```

The harness creates EFI, SYSTEM and DATA areas and exercises the same early-userspace → System Image → DATA → `luna-system-runtime` chain.

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

## Current implementation sequence

1. Fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
2. Real PC/UEFI boot validation and graphical desktop payload integration.
3. Complete durable boot/update state integration.
4. Finish LBP1 conformance and Ed25519 trust binding.
5. Formalize System Image/kernel compatibility and boot-success state.
6. Implement IPC/event transport, resource enforcement and device/volume integration.
7. Expand from development shell boot to real `.lbp` installation and ApplicationInstance launch/recovery.
8. Replace prototype `pre_exec` namespace setup with a production-safe child-creation primitive.

## License

Apache License 2.0.
