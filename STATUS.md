# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and subsequent accepted decisions are consolidated there.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is now in architecture-driven backend integration, end-to-end bring-up and hardening.

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
| Persistent state | Durable redb backend implemented under `DATA/system/state`; reopen/revision semantics tested; runtime/domain ownership integration remains |
| Update/checkpoint/rollback | Durable intent + per-operation applied/inflight journal implemented; physical domain-manager backends remain to be connected |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child spawn/poll/terminate/reap implemented; SystemRuntimeService now owns UserSession/process lifecycle |
| `luna-app-runtime` process launch | Real authorized launch boundary implemented; child namespace preparation and process-exit lifecycle reconciliation implemented as the current Linux prototype |
| `luna-boot.efi` | Working prototype reaches kernel + early userspace; QEMU harness now builds a real test System Image + DATA partition and hands off to `luna-system-runtime` |

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

`luna-namespace` provides private mount namespace creation, private mount propagation, OverlayFS composition over an immutable lower System Image, controlled read-only mapping application, private `/proc`, read-only `/sys`, an initially empty `/dev` tmpfs, and logical-root entry via `chroot`.

The backend remains an OS-specific primitive layer. It does not own Security policy, Bundle semantics, or application lifecycle.

### Security

`luna-security` models `Visibility` independently from `Read`/`Write`/`Execute`/`Use`/`Manage` and represents constrained decisions with typed `Constraint` values.

### System runtime process supervision

`luna-system-runtime` contains a real `ProcessSupervisor` based on `std::process::Command`/`Child`. It can spawn real child processes, poll them without blocking, terminate and reap them, and reconcile all finished children. `SystemRuntimeService` additionally owns UserSession creation and the initial interactive shell lifecycle.

### Application runtime

`luna-app-runtime` validates the Bundle/mapping contract, evaluates explicit Security requests before launch, delegates namespace construction to `luna-namespace`, and binds an `ApplicationInstance` to the process owned by `luna-system-runtime`. Process exit is translated into `Stopped`/`Failed` instance state and staging roots are cleaned up.

The current Linux namespace launch path uses `CommandExt::pre_exec` as an integration prototype. It must be replaced by a safer dedicated child-creation primitive before the runtime becomes multi-threaded/production-grade.

### Persistent state

`luna-state` uses `redb` as the accepted embedded durable backend at:

```text
DATA/system/state/luna-state.redb
```

Mutations and global revision are committed atomically in one redb transaction. No second Luna-specific WAL is layered over it.

### Update/checkpoint/rollback

`luna-update-manager` records durable intent before backend preparation and persists operation progress in:

```text
updates/<id>/phase
updates/<id>/plan
updates/<id>/applied
updates/<id>/inflight
```

Interrupted operations are reconciled from durable operation state, while rollback remains an explicit recovery/transaction action.

### Bootable development userspace

The QEMU harness under `boot/luna-boot/tests/ovmf/` now has a complete development bring-up chain:

```text
luna-boot.efi
    ↓
Linux kernel
    ↓
CPIO/gzip early userspace
    ↓
mount SYSTEM
    ↓
mount selected SquashFS System Image
    ↓
mount DATA
    ↓
switch_root
    ↓
luna-system-runtime
    ↓
UserSession
    ↓
interactive /bin/sh
```

The test harness creates separate EFI, SYSTEM and DATA partitions. The early-userspace script is intentionally a temporary bring-up implementation; it is not yet the final production `luna-init` component.

## RFC-0002

`docs/rfc/RFC-0002.md` is **Accepted** as Bundle Format v1. The reference implementation defines `LBP1`, a fixed 64-byte little-endian header, fixed 64-byte section entries, mandatory MANIFEST/PAYLOAD sections, optional RESOURCES/SIGNATURE sections, BLAKE3-256 integrity/content identity, TOML metadata, deterministic TAR payloads, zstd canonical compression, logical mappings, request-only capabilities, immutable installed bundles, and fail-closed parser validation.

The remaining RFC-related work is implementation conformance and production signature/trust integration, not reopening the accepted format without an explicit new decision.

## Current implementation sequence

1. Fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
2. Durable state ownership in system runtime/domain managers.
3. Concrete update backends connected to system/kernel/app managers.
4. LBP1 conformance and Ed25519 verification/trust binding.
5. System Image/kernel manifests, compatibility and persistent boot-success state.
6. IPC/event transport, cgroup/resource enforcement and device/volume integration.
7. End-to-end QEMU/Linux application launch and recovery tests.
8. Production-safe namespace child creation and runtime hardening.

## CI / supply chain

GitHub Actions checks workspace build/test/Clippy/release build and the separate UEFI target. SLSA provenance is configured for release subjects. Rustfmt remains advisory until the existing formatting debt is cleaned up.

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track reaches the Linux kernel and the real early-userspace handoff used by the QEMU development image. The bootloader remains outside the userspace Cargo workspace.
