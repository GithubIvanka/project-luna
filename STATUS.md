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
| Runtime resolver | Implemented in `luna-runtime`; resolves approved runtime artifacts without mounting or launching |
| Linux namespace/materialization | OverlayFS logical-root backend implemented; security-aware runtime preparation and real child launch integration implemented; production child-creation hardening remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; System Manager owns current/factory image+kernel state |
| Update/checkpoint/rollback | Durable intent + per-operation journal implemented; concrete domain mutation backends remain |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child spawn/poll/terminate/reap implemented; sole process owner |
| `luna-app-runtime` process launch | ApplicationInstance ↔ ProcessId binding, runtime-aware authorization and exit reconciliation implemented |
| `luna-boot.efi` | Development UEFI loader reaches early userspace/System Image/DATA handoff; normal graphical splash and Boot Menu Verbose Boot are implemented |
| x86_64 PC image | Reproducible GPT/UEFI development image builder implemented; graphical images require login + niri payload |
| Graphical desktop | UserSession login boundary exists; final niri + Noctalia payload/seat/device integration remains |

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

## Runtime architecture

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved glibc compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

`luna-runtime` resolves these typed choices to approved immutable runtime artifacts. It does not perform mounts, security authorization, or process launch. `luna-root-mapping`, `luna-security`, `luna-namespace`, and `luna-app-runtime` retain those boundaries.

Runtime use is represented in Security as `Resource::Runtime(kind)` + `Permission::Use`. Mapping plans can be bound to exactly one runtime kind. `ApplicationInstance` stores the selected `RuntimeSpec` for its lifecycle.

The final RFC-0002 manifest field for runtime selection is deliberately not introduced yet; that requires a dedicated Bundle/schema decision.

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

Pressing `B` during `luna-boot.efi` entry opens the exceptional text Boot Menu. Its `Verbose Boot` action suppresses the graphical splash, removes `quiet` and enables full kernel diagnostics for that boot. TTY/serial remains a development, diagnostic or recovery mechanism only.

## Bootable PC development image

`tools/build-pc-image.sh` produces:

```text
dist/luna-pc.img
```

with:

```text
EFI     128 MiB
SYSTEM  384 MiB
DATA    512 MiB
```

SYSTEM contains the versioned SquashFS image, its manifest, initramfs and kernel. DATA contains the persistent Luna state layout. `luna-system-runtime` is always built for `x86_64-unknown-linux-musl` for this image, and the builder refuses a non-static runtime.

The current graphical builder also refuses an image without:

```text
/usr/bin/luna-login
/usr/bin/niri-session
```

The guarded installer is:

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/<whole-disk> --yes
```

It does not write anything automatically and requires a second `ERASE-LUNA` confirmation.

## Application runtime

`luna-app-runtime` validates Bundle and mapping contracts, authorizes explicit Security requests before namespace materialization, delegates process ownership to `luna-system-runtime`, and binds each current bring-up `ApplicationInstance` to a supervised process. Process exit reconciles the corresponding `ApplicationInstance` to `Stopped` or `Failed` and cleans staging namespace resources.

The current Linux namespace launch path still uses Unix `CommandExt::pre_exec` as a development integration mechanism. Production hardening must replace complex post-fork setup with a dedicated child-creation primitive.

## Persistent state and updates

`luna-state` uses `redb` at:

```text
DATA/system/state/luna-state.redb
```

`luna-system-manager` owns logical current/factory System Image and kernel state. `luna-update-manager` remains the mutation coordinator and does not take ownership of domain semantics.

## Current implementation sequence

1. Connect `luna-runtime` to `luna-app-runtime` and materialize loader/library mappings inside the application namespace.
2. Validate the graphical PC image on real UEFI/QEMU and integrate the final niri + Noctalia payload.
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
```

These records supplement the Source of Truth; they do not silently override it.

## CI / supply chain

GitHub Actions checks workspace build/test/Clippy/release build and the separate UEFI target. The dedicated `Luna PC image` workflow additionally builds the x86_64 development disk image and uploads `luna-pc-x86_64` with SHA-256 checksums.
