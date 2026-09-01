# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and subsequent accepted decisions are consolidated there. Dated decision records under `docs/decisions/` preserve implementation history and boundaries.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.6-HZ** and is in **Phase 2 runtime/boot integration, PC bring-up and hardening**.

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
| Linux namespace/materialization | OverlayFS logical-root backend implemented; security-aware runtime preparation and real child launch integration implemented; production child-creation hardening remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state`; System Manager owns current/factory image+kernel state |
| Update/checkpoint/rollback | Durable intent + per-operation journal implemented; concrete domain mutation backends remain |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` process supervision | Real child spawn/poll/terminate/reap implemented; sole process owner |
| `luna-app-runtime` process launch | ApplicationInstance ↔ ProcessId binding, runtime-aware authorization and exit reconciliation implemented |
| `luna-boot.efi` | Development UEFI loader reaches early userspace/System Image/DATA handoff |
| x86_64 PC image | Reproducible GPT/UEFI development image builder implemented; CI artifact workflow added |
| Graphical desktop | Session boundary exists; final niri + Noctalia payload/seat/device integration remains |

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

## Implemented runtime contract

The shared runtime value contract is:

```text
RuntimeKind
├── Luna   → native Luna userspace / musl
├── Glibc  → approved glibc compatibility runtime
└── Bundle → Bundle-private runtime where permitted
```

Runtime use is represented in Security as `Resource::Runtime(kind)` + `Permission::Use`. Mapping plans can be bound to exactly one runtime kind. `ApplicationInstance` stores the selected `RuntimeSpec` for its lifecycle.

The final RFC-0002 manifest field for runtime selection is deliberately not introduced yet; that requires a dedicated Bundle/schema decision.

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

The normal PC boot target uses a quiet `tty0` console. The B-key boot menu still exposes the explicit serial development target. Early userspace prefers `LABEL=LUNA-SYSTEM` and `LABEL=LUNA-DATA`, with `/dev/sda2`/`/dev/sda3` retained only as compatibility fallback for old QEMU disks.

The guarded installer is:

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/<whole-disk> --yes
```

It does not write anything automatically and requires a second `ERASE-LUNA` confirmation.

## Development graphical session

The session lifecycle remains:

```text
Starting
  ↓
Authenticating
  ↓
Active
  ↓
Wayland
  ↓
niri
  ↓
Noctalia Shell
```

The current image ships `/usr/bin/luna-session` as a small session selection boundary. Without `niri-session` it falls back to `/bin/sh`, keeping the image usable while the full graphical runtime tree is integrated.

## Application runtime

`luna-app-runtime` validates Bundle and mapping contracts, authorizes explicit Security requests before namespace materialization, and delegates process ownership to `luna-system-runtime`. Process exit reconciles the corresponding `ApplicationInstance` to `Stopped` or `Failed` and cleans staging namespace resources.

The current Linux namespace launch path still uses Unix `CommandExt::pre_exec` as a development integration mechanism. Production hardening must replace complex post-fork setup with a dedicated child-creation primitive.

## Persistent state and updates

`luna-state` uses `redb` at:

```text
DATA/system/state/luna-state.redb
```

`luna-system-manager` owns logical current/factory System Image and kernel state. `luna-update-manager` remains the mutation coordinator and does not take ownership of domain semantics.

## Bundle format

`docs/rfc/RFC-0002.md` is **Accepted** as Bundle Format v1. Remaining work is conformance, deterministic verification, Ed25519 signature/trust binding and installation-stage integration.

## Current implementation sequence

1. Fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
2. Validate the reproducible PC image on real UEFI/QEMU and integrate the final graphical runtime payload.
3. Complete durable boot/update state integration.
4. Finish LBP1 conformance and Ed25519 trust binding.
5. Formalize System Image/kernel compatibility and persistent boot-success state.
6. Implement IPC/event transport, resource enforcement and device/volume integration.
7. Expand from shell boot to real `.lbp` installation and ApplicationInstance launch/recovery.
8. Replace prototype `pre_exec` namespace setup with a production-safe child-creation primitive.

## Decision records

Current implementation decisions include:

```text
docs/decisions/2026-09-01-RUNTIME-INTEGRATION.md
docs/decisions/2026-09-01-RUNTIME-CONTRACT.md
docs/decisions/2026-09-01-PC-BUILD.md
```

These records supplement the Source of Truth; they do not silently override it.

## CI / supply chain

GitHub Actions checks workspace build/test/Clippy/release build and the separate UEFI target. The dedicated `Luna PC image` workflow additionally builds the x86_64 development disk image and uploads it as `luna-pc-x86_64` with SHA-256 checksums.
