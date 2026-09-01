# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and subsequent accepted decisions are consolidated there. Dated decision records under `docs/decisions/` preserve implementation history and boundaries.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.1–1.6-HZ** and is in **Phase 2 runtime/boot integration, PC bring-up, desktop integration and hardening**.

The Phase 2 stacked integration chain has been consolidated into `main` as commit `9013a8efb305c1b98587b0fc356cf79e473dcd39`. Old stacked PRs were closed as superseded; new work continues from the single `development` branch.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Post-1.6 accepted architecture decisions | Consolidated into the Source of Truth |
| Repository/Cargo audit | Completed baseline; repeated during implementation passes |
| Crate map | Synchronized with repository |
| Foundation/domain APIs | Implemented baseline |
| Manager/runtime APIs | Implemented baseline |
| Typed runtime contract | Implemented: `luna`, `glibc`, `bundle` |
| Runtime ↔ mapping ↔ security | Implemented contract and pre-materialization authorization |
| Runtime resolver | Not part of Luna architecture; removed. Runtime choice remains with existing runtime/mapping/security boundaries. |
| Linux namespace/materialization | OverlayFS logical-root backend implemented; security-aware runtime preparation and real child launch integration implemented; production child-creation hardening remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; System Manager owns current/factory image+kernel state |
| Update/checkpoint/rollback | Durable intent + per-operation journal implemented; concrete domain mutation backends remain |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child spawn/poll/terminate/reap implemented; sole process owner |
| `luna-app-runtime` process launch | ApplicationInstance ↔ ProcessId binding, runtime-aware authorization and exit reconciliation implemented |
| `luna-boot.efi` | Development UEFI loader reaches early userspace/System Image/DATA handoff; GUI splash and full Boot Menu action set are represented; recovery/factory/external boot backends remain to be wired |
| x86_64 PC image | Reproducible GPT/UEFI development image builder implemented; graphical images require login + niri payload |
| Graphical desktop | UserSession login boundary exists; final niri + Noctalia payload/seat/device integration remains |

## Boot and user experience

Normal boot is graphical and quiet:

```text
Power
 ↓
UEFI
 ↓
luna-boot.efi
 ↓
Luna GUI boot splash
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

There is no normal TTY login, console shell fallback, or `luna-session` component.

Pressing `B` during `luna-boot.efi` entry opens the exceptional text Boot Menu. It contains:

```text
Continue to Luna
System Image: <version>
Recovery Environment
Factory Environment
Boot from USB / External Device
Verbose Boot
```

`Verbose Boot` suppresses the GUI splash and enables full kernel diagnostics for that boot. TTY/serial remains a development, diagnostic or recovery mechanism only.

The current development image represents Recovery/Factory/External Boot as explicit menu actions but does not pretend those backends are complete yet; selecting an unavailable backend returns a controlled boot error.

## Runtime architecture

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved glibc compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

`RuntimeKind` is a shared typed value. There is no generic `luna-runtime` subsystem. Runtime launch is owned by `luna-app-runtime`, with `luna-root-mapping`, `luna-security` and `luna-namespace` providing the surrounding policy/materialization contracts.

The final RFC-0002 manifest field for runtime selection is deliberately not introduced yet; that requires a dedicated Bundle/schema decision.

## Current implementation sequence

1. Extend `luna-app-runtime` to resolve `RuntimeKind` using existing mapping and security contracts and materialize loader/library mappings inside the application namespace.
2. Implement the final graphical `luna-login` payload and niri + Noctalia desktop runtime tree.
3. Finish fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
4. Complete durable boot/update state integration and persistent boot-success state.
5. Finish LBP1 conformance and Ed25519 verification/trust binding.
6. Implement IPC/event transport, resource enforcement and device/volume integration.
7. Expand to real `.lbp` installation and ApplicationInstance launch/recovery.
8. Replace prototype `pre_exec` namespace setup with a production-safe child-creation primitive.

## Decision records

Current implementation decisions include:

```text
docs/decisions/2026-09-01-RUNTIME-INTEGRATION.md
docs/decisions/2026-09-01-RUNTIME-CONTRACT.md
docs/decisions/2026-09-01-PC-BUILD.md
docs/decisions/2026-09-01-GRAPHICAL-BOOT-SESSION.md
docs/decisions/2026-09-01-GIT-WORKFLOW.md
docs/decisions/2026-09-01-MAIN-PROTECTION.md
docs/decisions/2026-09-01-RUNTIME-BOUNDARIES.md
```
