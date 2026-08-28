# Project Luna — Status

Last updated: 2026-08-28

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. This file is a status snapshot only.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and has moved into architecture-driven API implementation and integration-contract hardening.

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
| Crate map | Established |
| Foundation/domain API pass | Completed |
| Manager/runtime API baseline | Completed |
| Integration-contract hardening | **Completed as a prototype pass** |
| Namespace/materialization prototype | **Completed as a pure contract prototype** |
| Persistence/update transaction prototype | **Completed as an in-memory atomic prototype** |
| Bundle Format v1 | RFC-0002 remains Draft / Proposal; implementation notes added |
| luna-boot | Parallel prototype work; outside userspace workspace |

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

These crates are a mix of implemented domain/API contracts and intentionally incomplete subsystem scaffolds. API presence must not be confused with a finished operating-system implementation.

## Integration hardening completed

- `luna-root-mapping`: deterministic namespace materialization contract added; no kernel mounts are performed.
- `luna-security`: explicit-grant `StaticPolicyAuthority` added for deterministic policy tests; no privileged/root-user abstraction introduced.
- `luna-bundle`: logical/source path validation hardened against traversal, absolute payload sources and duplicate logical resources.
- `luna-config`: existing application precedence remains user/application override → application default → system default.
- `luna-user-session` + `luna-app-runtime`: application launch prototype now requires an active session and a valid mapping for every declared resource; instance stop/failure lifecycle is explicit.
- `luna-event`: deterministic in-memory FIFO delivery prototype added; broker/async transport remains a higher-level concern.
- `luna-state`: public `MemoryStateStore` added with atomic revision-checked transactions and validation before mutation.
- `luna-update-manager`: transactional executor prototype records an update plan through one atomic state transaction; real image/bundle mutation remains deferred.
- RFC-0002: implementation-boundary companion notes added without silently accepting the proposed `.lbp` wire format.

## CI / supply-chain

- Rust CI now checks formatting, workspace check/test, clippy, release build and the separate `luna-boot` UEFI target.
- `luna-boot.efi` is uploaded as a CI artifact.
- The SLSA workflow now hashes real Luna release subjects instead of demonstration `artifact1`/`artifact2` files and references the versioned generic generator.

## Important boundaries

`luna-app-manager` does not own normal application execution.

`luna-system-manager` owns the system state model; `luna-update-manager` executes mutations.

`luna-kernel-manager` owns kernel modeling/compatibility queries; update execution remains with `luna-update-manager`.

`luna-security` remains the central policy authority. Low-level kernel/filesystem/runtime mechanisms enforce policy decisions.

`luna-system-runtime` is the single system-wide runtime supervising multiple `UserSession` instances. `luna-app-runtime` owns application execution and `ApplicationInstance` lifecycle.

## Explicitly still deferred

- production Linux namespace/mount implementation;
- durable on-disk state/WAL/snapshot backend;
- real update checkpoint/rollback protocol;
- final IPC transport;
- final async event broker/channel implementation;
- System Image specification and implementation;
- kernel compatibility resolver implementation;
- boot-state metadata;
- production `luna-boot.efi` Linux boot protocol handoff;
- final `.lbp` wire/archive implementation and RFC-0002 acceptance;
- signature/trust verification implementation;
- final CLI grammar and GUI.

## Verification note

Changes were made directly in the repository. The connected environment used for this session does not provide a reliable local Cargo execution path, so no successful local `cargo test --workspace` claim is made. GitHub Actions is now configured to perform the workspace and UEFI builds on push/PR.

## Next work

1. Review CI results and fix any compiler/API incompatibilities surfaced by the new checks.
2. Promote the pure prototypes only when their backend contracts are accepted.
3. Resolve the explicit RFC-0002 decision list before calling Bundle Format v1 Accepted.
4. Continue toward real namespace, persistence/update, and boot compatibility implementations.
