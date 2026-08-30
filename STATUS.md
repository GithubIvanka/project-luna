# Project Luna — Status

Last updated: 2026-08-30

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is now in architecture-driven backend implementation and high-risk integration work.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Repository/Cargo audit | Completed |
| Crate map | Synchronized with repository |
| Foundation/domain APIs | Completed baseline |
| Manager/runtime APIs | Completed baseline |
| Integration contracts | Prototype completed |
| Linux namespace/materialization | **Logical-root backend implemented on top of Linux mount namespaces; application-runtime preparation boundary added; security/device integration remains** |
| Persistent state | **Durable redb backend implemented under DATA/system/state; crash/reopen contract tested** |
| Update/checkpoint/rollback | **Checkpointed orchestration engine implemented; domain-manager backends remain to be connected** |
| Bundle Format v1 | RFC-0002 Draft / Proposal; **candidate decision set added, not yet accepted** |
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

There is no separate `lunad` architecture component and no separate Session Manager. `UserSession` is the combined user/session domain entity. `luna-system-runtime` is the single system-wide runtime/supervisor.

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

`luna-namespace` now provides:

- private Linux mount namespace creation;
- private mount propagation;
- System Image base-root bind mounting;
- creation of conventional Linux root directories;
- controlled read-only mapping application;
- explicit writable mapping primitive for already-authorized callers;
- private `/proc` and read-only `/sys` views;
- private empty `/dev` tmpfs, ready for authorized device-manager binds;
- `chroot` into the prepared logical root.

The crate remains a low-level OS backend. It does not decide policy, own application lifecycle, or replace `luna-root-mapping`.

### Application runtime integration

`luna-app-runtime` now has an explicit namespace-preparation boundary in addition to its in-memory lifecycle model. The runtime validates the bundle/mapping contract, tracks `ApplicationInstance`, and can ask `luna-namespace` to materialize the logical root for an existing instance without moving process creation or policy ownership into the namespace crate.

The final process-exec/supervision step remains outside this preparation API by design.

### Persistent state

`luna-state` now has a durable `RedbStateStore` using `redb` as the accepted first embedded backend. The database defaults to:

```text
DATA/system/state/luna-state.redb
```

State mutations and the global revision are committed together in one ACID transaction. The implementation deliberately does not add a second Luna-specific WAL on top of redb. Reopen/persistence and revision semantics are covered by tests.

### Update/checkpoint/rollback

`luna-update-manager` now contains `UpdateEngine` with:

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

Failures trigger reverse-order rollback through the domain `UpdateBackend`. A non-terminal durable phase can be reconciled after interruption. The engine remains independent of concrete filesystem/domain mutation; domain managers provide the backend.

## RFC-0002 work

`docs/rfc/RFC-0002.md` remains the normative draft/proposal document. A new companion file, `docs/rfc/RFC-0002-V1-DECISION-SET.md`, records one coherent candidate for the open v1 decisions without silently promoting it to architecture.

The candidate currently proposes:

- structured `LBP1` container;
- fixed little-endian header and section table;
- BLAKE3-256 content hashing;
- TOML manifest;
- v1 bundle types limited to `application` and `component`;
- Luna semantic versioning;
- bundle-relative payload paths and logical mappings;
- opaque, request-only capability declarations;
- optional Ed25519 signature section;
- deterministic tar semantics inside the payload section with zstd compression;
- fail-closed handling of unknown versions, bad paths, overlaps and integrity failures.

The final acceptance gate still requires a tested writer/reader, payload validation, integrity tests, signature coverage tests and a decision-history update. Until then, the candidate is not an accepted architecture decision.

## Remaining work in the current sequence

1. Connect the logical-root backend to `luna-security` authorization and real process creation/supervision.
2. Connect update backends to `luna-system-manager`, `luna-kernel-manager` and `luna-app-manager` without moving lifecycle ownership into `luna-update-manager`.
3. Turn the RFC-0002 candidate into a tested `.lbp` codec and then promote the agreed decisions into the normative RFC.
4. Continue production signature/trust integration.
5. Formalize System Image/kernel compatibility and persistent boot-state metadata.
6. Select final IPC/event transport.
7. Tune resource enforcement and device/volume integration.
8. Add end-to-end integration tests.

## CI / supply chain

GitHub Actions now checks formatting, workspace build/test/clippy/release build, and the separate UEFI boot target. The SLSA provenance workflow has explicit least-privilege build permissions and generates provenance for release subjects.

## Explicitly still deferred

- final security policy/runtime enforcement integration;
- final filtered `/dev` device population;
- final IPC transport and async event transport;
- System Image manifest specification and compatibility implementation;
- persistent boot-state metadata and userspace boot-success confirmation;
- final `.lbp` wire/archive implementation and RFC-0002 acceptance;
- production signature/trust chain;
- final CLI grammar/alias persistence;
- GUI implementation;
- device automount backend;
- final resource policy tuning.
