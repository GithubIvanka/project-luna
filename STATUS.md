# Project Luna — Status

Last updated: 2026-08-31

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only. Post-1.6 accepted decisions are recorded in `docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md` pending safe consolidation into the large Source of Truth document.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has entered architecture-driven backend implementation, integration, and format hardening.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Post-1.6 accepted clarifications | Recorded in architecture amendment; pending merge into SoT body |
| Repository/Cargo audit | Completed baseline |
| Crate map | Synchronized with repository |
| Foundation/domain APIs | Implemented baseline |
| Manager/runtime APIs | Implemented baseline |
| Linux namespace/materialization | Backend implemented; runtime preparation boundary exists; security/device enforcement still being integrated |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; reopen/revision semantics tested |
| Update/checkpoint/rollback | Checkpointed orchestration engine implemented; physical domain-manager backends still being connected |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec implemented and under hardening** |
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

These crates represent current architecture boundaries. Their presence does not mean every backend is production-complete.

## Current runtime model

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

There is no separate `lunad` architecture component and no separate normal Session Manager. `UserSession` is the combined user/session domain entity. `luna-system-runtime` is the single system-wide runtime/supervisor.

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

The logical application root remains a conventional Linux-compatible `/`, while the physical DATA layout stays Luna-native. Application namespace composition is controlled by mapping plus security policy.

## Implemented backend work

### Linux namespace + logical root

`luna-namespace` provides:

- private Linux mount namespace creation;
- private mount propagation;
- System Image base-root binding;
- conventional logical-root directory preparation;
- controlled read-only mapping application;
- explicit writable mapping primitive for an already-authorized caller;
- private `/proc` and read-only `/sys` views;
- private empty `/dev` tmpfs, ready for authorized device-manager binds;
- `chroot` into the prepared logical root.

The backend remains an OS-specific primitive layer. It does not own security policy, application lifecycle, or Bundle semantics.

### Application runtime integration

`luna-app-runtime` validates bundle/mapping contracts, tracks `ApplicationInstance`, requires an active `UserSession`, and exposes an explicit namespace-preparation boundary. The final process creation/exec and system supervision remain outside this preparation API.

### Persistent state

`luna-state` now has `RedbStateStore` using `redb` as the accepted first embedded backend. The default system state path is:

```text
DATA/system/state/luna-state.redb
```

State mutations and the global revision are committed together in one transactional operation. No second Luna-specific WAL is layered over the database.

### Update/checkpoint/rollback

`luna-update-manager` contains `UpdateEngine` with:

```text
prepare
  ↓
checkpoint
  ↓
apply
  ↓
verify
  ↓
commit
```

Failures trigger reverse-order rollback through the domain `UpdateBackend`. Interrupted non-terminal operations can be reconciled. Domain managers remain owners of their own state.

## RFC-0002

`docs/rfc/RFC-0002.md` is **Accepted** as Bundle Format v1.

The accepted format defines:

- `LBP1` fixed 64-byte little-endian header;
- fixed 64-byte section entries;
- required MANIFEST and PAYLOAD sections;
- optional RESOURCES and SIGNATURE sections;
- BLAKE3-256 integrity/content identity;
- TOML manifest;
- `application` and `component` bundle types;
- deterministic TAR payload with canonical metadata;
- zstd canonical payload compression;
- logical mappings with Bundle-relative sources;
- capabilities as requests only;
- optional Ed25519 signatures;
- immutable installed bundles and version coexistence;
- fail-closed malformed-input handling;
- delta updates outside RFC-0002.

The current `luna-bundle` implementation is the reference implementation boundary. Remaining work is implementation hardening and test coverage, not re-opening the accepted wire-format decisions without a new RFC/decision record.

## Current implementation sequence

1. Finish security authorization + device filtering around namespace materialization.
2. Correct the remaining namespace materialization edge cases and add integration coverage.
3. Connect persistent state to runtime/system-manager ownership.
4. Connect update backends to domain managers and add precise interruption/reconciliation state.
5. Harden LBP1 parser/writer against malformed input and complete integrity/signature coverage.
6. Formalize System Image and kernel manifests/compatibility and persistent boot-state confirmation.
7. Select final IPC/event transport and implement production event persistence boundaries.
8. Integrate cgroups/resource enforcement and volume/device manager backends.
9. Add end-to-end QEMU/Linux integration coverage.

## Known implementation gaps

- namespace materialization currently uses low-level Linux mount/chroot primitives but is not yet the complete production application launcher;
- security policy is not yet wired into every runtime mapping operation;
- device population of `/dev` is intentionally not implemented as unrestricted host access;
- update interruption state does not yet persist per-operation applied progress, so reconciliation is conservative;
- LBP1 currently exposes signature bytes but full Ed25519 verification/trust binding remains a subsequent security implementation step;
- final System Image/kernel specification is not yet complete;
- final CLI grammar and aliases remain open;
- device automount backend remains open.

## CI / supply chain

GitHub Actions checks workspace build/test/clippy/release build and the separate UEFI target. SLSA provenance is configured for release subjects. Rustfmt is currently advisory because legacy formatting debt remains in the repository; it should become blocking after a dedicated formatting cleanup.

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track reaches the Linux kernel plus test init and `sh`. The bootloader is not being redesigned as part of the current Bundle work.
