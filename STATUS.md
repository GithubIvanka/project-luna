# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and the subsequent accepted decisions are consolidated there.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has entered architecture-driven backend implementation, integration, and format hardening.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Post-1.6 accepted architecture decisions | Consolidated into the Source of Truth |
| Repository/Cargo audit | Completed baseline; repeated during implementation passes |
| Crate map | Synchronized with repository |
| Foundation/domain APIs | Implemented baseline |
| Manager/runtime APIs | Implemented baseline |
| Linux namespace/materialization | OverlayFS-based logical-root backend implemented; security-aware runtime preparation boundary added; device/process integration remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; reopen/revision semantics tested |
| Update/checkpoint/rollback | Durable intent + per-operation applied/inflight journal implemented; physical domain-manager backends remain to be connected |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child-process spawn/poll/terminate/reap backend implemented; namespace integration remains |
| `luna-boot.efi` | Working prototype reaches kernel + test init + `sh`; production hardening remains |

## Current workspace

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

### Application runtime

`luna-app-runtime` has a security-aware namespace preparation boundary. Production callers can supply the central `PolicyAuthority` and explicit authorization requests; non-`Allow` decisions fail closed before namespace materialization. Process creation and supervision are now provided by `luna-system-runtime`; binding the two through the higher-level runtime orchestration remains next.

### System runtime process supervision

`luna-system-runtime` now contains a real `ProcessSupervisor` based on `std::process::Command`/`Child`. It can spawn authorized child processes, poll them without blocking, terminate and reap them, and reconcile all finished children. The supervisor intentionally does not perform Security decisions or namespace construction; those remain separate architectural boundaries.

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

## RFC-0002

`docs/rfc/RFC-0002.md` is **Accepted** as Bundle Format v1. The reference implementation defines `LBP1`, a fixed 64-byte little-endian header, fixed 64-byte section entries, mandatory MANIFEST/PAYLOAD sections, optional RESOURCES/SIGNATURE sections, BLAKE3-256 integrity/content identity, TOML metadata, deterministic TAR payloads, zstd canonical compression, logical mappings, request-only capabilities, immutable installed bundles, and fail-closed parser validation.

The remaining RFC-related work is implementation conformance and production signature/trust integration, not reopening the accepted format without an explicit new decision.

## Current implementation sequence

1. Connect `luna-system-runtime` process supervision to `luna-app-runtime` launch orchestration without moving Security or namespace ownership.
2. Finish fine-grained security-to-mapping/device authorization and filtered `/dev` population.
3. Connect update backends to `luna-system-manager`, `luna-kernel-manager`, and `luna-app-manager`.
4. Finish LBP1 conformance tests and Ed25519 verification/trust binding.
5. Connect durable state to runtime/domain-manager ownership.
6. Formalize System Image/kernel manifests, compatibility, and boot-success state.
7. Implement final IPC/event transport, cgroup/resource enforcement, filtered device population, and end-to-end QEMU/Linux tests.

## CI / supply chain

GitHub Actions checks workspace build/test/Clippy/release build and the separate UEFI target. SLSA provenance is configured for release subjects. Rustfmt remains advisory until the existing formatting debt is cleaned up.

The latest pushes have fresh CI runs in progress; conclusions are not assumed until GitHub reports them.

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track reaches the Linux kernel plus test init and `sh`. The bootloader is not being redesigned as part of the current Bundle work.
