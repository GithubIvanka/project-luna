# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and subsequent accepted decisions are consolidated there. The dated runtime integration record under `docs/decisions/` preserves the current implementation-boundary decisions.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has entered architecture-driven backend integration, end-to-end bring-up and hardening.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Post-1.6 accepted architecture decisions | Consolidated into the Source of Truth |
| Repository/Cargo audit | Completed baseline; repeated during implementation passes |
| Crate map | Synchronized with repository |
| Foundation/domain APIs | Implemented baseline |
| Manager/runtime APIs | Implemented baseline |
| Linux namespace/materialization | OverlayFS logical-root backend implemented; security-aware runtime preparation boundary and real child launch integration implemented; filtered device integration remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; System Manager now owns current/factory image+kernel state and runtime can attach it |
| Update/checkpoint/rollback | Durable intent + per-operation applied/inflight journal implemented; concrete domain mutation backends remain to be connected |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child spawn/poll/terminate/reap implemented; `SystemRuntimeService` is the sole process owner and owns UserSession/process lifecycle |
| `luna-app-runtime` process launch | Security/mapping validation plus real child launch through `SystemRuntimeService`; ApplicationInstance ↔ ProcessId binding and exit reconciliation implemented |
| `luna-boot.efi` | Working prototype reaches kernel + early userspace; development QEMU path now builds SYSTEM + DATA and hands off to `luna-system-runtime` |

## Current storage model

```text
EFI
SYSTEM
DATA
├── system/{apps,drivers,libs,volumes,config,state}
│   └── state/luna-state.redb
├── users/<user>/{home,data,config}
└── cache
SWAP / ZRAM
```

The logical application root remains a conventional Linux-compatible `/`, while the physical DATA layout stays Luna-native. Namespace composition is controlled by mapping and security policy.

## Implemented backend work

### Linux namespace + logical root

`luna-namespace` provides private mount namespace creation, private mount propagation, OverlayFS composition over an immutable lower System Image, controlled mapping application, private `/proc`, read-only `/sys`, an initially empty `/dev` tmpfs, device exposure primitives and logical-root entry via `chroot`.

### Security

`luna-security` models `Visibility` independently from `Read`/`Write`/`Execute`/`Use`/`Manage` and represents constrained decisions with typed `Constraint` values.

### System runtime process supervision

`luna-system-runtime` contains a real `ProcessSupervisor` based on `std::process::Command`/`Child`. `SystemRuntimeService` is the sole process owner and exposes typed spawn/poll/terminate operations to upper runtime layers. Its normal `supervise()` path only consumes processes that belong to UserSession shell lifecycle, preventing it from accidentally reaping application processes owned through the same supervisor.

### Application runtime

`luna-app-runtime` validates Bundle/mapping contracts, evaluates explicit Security requests before namespace materialization, delegates Linux process ownership to `luna-system-runtime`, and binds each current bring-up `ApplicationInstance` to a supervised process. Process exit is translated into `Stopped`/`Failed` instance state and staging roots are cleaned up.

The current Linux namespace launch path uses Unix `CommandExt::pre_exec` as a bring-up implementation. Rust documents that this callback runs after `fork` in a constrained child context, so replacing it with a dedicated child-creation primitive remains a production-hardening task. citeturn607694search2

### Persistent state

`luna-state` uses `redb` as the accepted embedded durable backend at:

```text
DATA/system/state/luna-state.redb
```

Mutations and global revision are committed atomically in one redb transaction. `luna-system-manager` now owns durable logical System State including current/factory System Image and current/factory kernel, while `luna-system-runtime` can attach that manager without taking ownership of update execution.

### Update/checkpoint/rollback

`luna-update-manager` records durable intent before backend preparation and persists operation progress in:

```text
updates/<id>/phase
updates/<id>/plan
updates/<id>/applied
updates/<id>/inflight
```

Interrupted operations are reconciled from durable operation state, while rollback remains an explicit recovery/transaction action. Concrete mutation adapters for domain managers are the next implementation step.

### Bootable development userspace

The QEMU harness under `boot/luna-boot/tests/ovmf/` now has a development bring-up chain:

```text
luna-boot.efi
    ↓
Linux kernel
    ↓
early initramfs
    ↓
SYSTEM
    ↓
SquashFS System Image
    ↓
DATA
    ↓
switch_root
    ↓
luna-system-runtime
    ↓
UserSession
    ↓
interactive shell
```

The repository provides `build-userspace.sh`, `luna-init` and `build-and-run.sh` to assemble this path reproducibly when the host supplies QEMU/OVMF, a Linux kernel, and a static BusyBox.

## RFC-0002

`docs/rfc/RFC-0002.md` is **Accepted** as Bundle Format v1. The reference implementation defines `LBP1`, a fixed 64-byte little-endian header, fixed 64-byte section entries, mandatory MANIFEST/PAYLOAD sections, optional RESOURCES/SIGNATURE sections, BLAKE3-256 integrity/content identity, TOML metadata, deterministic TAR payloads, zstd canonical compression, logical mappings, request-only capabilities, immutable installed bundles, and fail-closed parser validation.

The remaining RFC-related work is implementation conformance and production signature/trust integration, not reopening the accepted format without an explicit new decision.

## Current implementation sequence

1. Fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
2. Complete durable state integration with runtime/domain-manager ownership and boot/update state.
3. Connect concrete update backends to system/kernel/app managers.
4. Finish LBP1 conformance and Ed25519 verification/trust binding.
5. Formalize System Image/kernel manifests, compatibility and persistent boot-success state.
6. Implement IPC/event transport, cgroup/resource enforcement and device/volume integration.
7. Expand the QEMU path from shell bring-up to actual `.lbp` Bundle installation and ApplicationInstance launch/recovery.
8. Replace prototype `pre_exec` namespace setup with a production-safe child-creation primitive.

## Decision records

The current accepted implementation-boundary decisions are additionally recorded in:

```text
docs/decisions/2026-09-01-RUNTIME-INTEGRATION.md
```

That record explicitly preserves the rule that `luna-system-runtime` owns process supervision, `luna-app-runtime` does not create a second supervisor, and `luna-system-manager` owns persistent system state.

## CI / supply chain

GitHub Actions checks workspace build/test/Clippy/release build and the separate UEFI target. SLSA provenance is configured for release subjects. Rustfmt remains advisory until the existing formatting debt is cleaned up.

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track reaches the Linux kernel and the development early-userspace/System-Image/DATA handoff. The bootloader remains outside the userspace Cargo workspace.
