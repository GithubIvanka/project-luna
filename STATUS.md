# Project Luna — Status

Last updated: 2026-08-29

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ**. The project is now in architecture-driven backend implementation and high-risk integration work.

### Phase status

| Phase | Status |
|---|---|
| 1.1 | Accepted and consolidated |
| 1.2 | Accepted and consolidated |
| 1.3 | Accepted and consolidated |
| 1.4 | Accepted and consolidated |
| 1.5 | Accepted and consolidated |
| 1.6 | Accepted through 1.6-HZ and consolidated |
| Repository/Cargo audit | Completed |
| Crate map | Synchronized with repository |
| Foundation/domain API pass | Completed |
| Manager/runtime API baseline | Completed |
| Integration-contract hardening | Prototype completed |
| Namespace/materialization | Contract prototype completed; real Linux backend next |
| Persistence/update transaction | In-memory atomic prototype completed; durable backend next |
| Bundle Format v1 | RFC-0002 still Draft / Proposal |
| `luna-boot.efi` | Working prototype reaches kernel + test init + `sh`; production hardening remains |

## Current workspace

```text
luna-common
luna-fs
luna-root-mapping
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
└── system/{apps,drivers,libs,volumes,config,state}
    users/<user>/{home,data,config}
    cache
SWAP / ZRAM
```

The logical application root remains a conventional Linux-compatible `/`, while the physical DATA layout stays Luna-native. Application namespace composition is controlled by mapping plus security policy.

## Completed implementation-contract work

- `luna-root-mapping`: deterministic mapping/materialization contract with file mappings by default and explicit subtree mappings for suitable semantic classes.
- `luna-security`: central policy boundary with explicit authorization decisions and structured restrictions.
- `luna-bundle`: internal Bundle model and manifest/resource validation.
- `luna-config`: layered user/application/system configuration model.
- `luna-user-session` + `luna-app-runtime`: lifecycle contract for UserSession and ApplicationInstance.
- `luna-event`: event domain and bounded-delivery contract.
- `luna-state`: synchronous state abstraction, global revision and atomic transaction contract.
- `luna-update-manager`: update-plan/executor contract and in-memory atomic prototype.

## Current implementation direction

### Real Linux namespace/materialization backend

Use existing Linux mechanisms as primitives rather than inventing a custom VFS. The target behavior remains:

```text
logical Linux root
    ↓
per-namespace mapping
    ↓
controlled mounts/bind mounts
    ↓
security-enforced resource view
```

The application must see a normal Linux-compatible filesystem interface and must not be exposed to an artificial container-style filesystem layout.

### Persistent state backend

The state abstraction remains backend-agnostic. The first durable implementation direction is a small embedded transactional key/value backend; the backend choice is an implementation decision, not a new Luna architectural boundary. State ownership remains above the raw storage mechanism.

### Update/rollback engine

`luna-update-manager` remains the execution side. `luna-system-manager` and other managers own domain state/query models. Update execution must be checkpoint-aware, interruption-safe and explicitly reversible where rollback is supported.

### Bundle Format v1

`luna-bundle` owns Bundle domain/format concerns. `luna-app-manager` owns install/update/remove/import lifecycle. `.lbp` is transport/archive representation only. RFC-0002 is not accepted yet.

### Security / signatures

The architecture requires separation of signature verification, trust decision and runtime permission. Production publisher/repository signature verification and trust storage are still implementation work.

## `luna-boot.efi`

Bootloader work is maintained separately under `boot/luna-boot/`. The current prototype has progressed to real kernel loading and a test init handoff that reaches `sh` in the other boot-focused development track. This work is not represented as a userspace crate.

## CI / supply chain

GitHub Actions is configured for Rust workspace verification and the separate UEFI boot target. SLSA provenance generation is also enabled.

## Explicitly still deferred

- production Linux namespace/mount implementation;
- durable on-disk state backend;
- production update/checkpoint/rollback protocol;
- final IPC transport;
- final async event transport/broker;
- System Image manifest specification and compatibility implementation;
- persistent boot-state metadata;
- final `.lbp` wire/archive implementation and RFC-0002 acceptance;
- production signature/trust chain;
- final CLI grammar/alias persistence;
- GUI implementation;
- device automount backend;
- final resource policy tuning.

## Next work

1. Implement and test the real Linux namespace + logical-root materialization backend.
2. Implement durable `luna-state` persistence and crash-safe recovery.
3. Implement the real update/checkpoint/rollback engine.
4. Resolve and accept RFC-0002, then implement `.lbp`.
5. Continue production security/signature integration.
