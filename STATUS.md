# Project Luna — status

**Revision:** 2026-09-15

`docs/ARCHITECTURE.md` is the architectural authority. This file records implementation status only.

## Implemented foundation

- Rust workspace and shared domain types.
- LBP1 Bundle codec with BLAKE3, TAR, zstd, TOML and Ed25519 support.
- Logical mapping model in `luna-root-mapping`.
- Linux namespace and Landlock primitives in `luna-namespace`.
- Authorization model in `luna-security`.
- Durable `redb` state backend in `luna-state`.
- System target/state model in `luna-system-manager`.
- Update/checkpoint orchestration in `luna-update-manager`.
- Process supervision and session APIs in `luna-system-runtime`.
- Application plan and instance lifecycle in `luna-app-runtime`.
- UEFI discovery, target selection, kernel loading, boot menu, external boot and handoff code.
- Direct-memory `luna-init` kernel plumbing and FD 3 handoff support.
- x86_64 PC image builder using `LUNA-SYS` and `LUNA-DATA` labels.

## Partial implementation

`luna-init` currently validates FD 3 and keeps the initial process alive, but the full architecture-defined logical-root construction and child startup of `luna-system-runtime` are not implemented yet.

The bootloader has discovery/selection/handoff machinery, but complete end-to-end boot success, persistent fallback behavior and production hardening still require integration testing.

Application runtime has planning, authorization and namespace primitives, but production-safe child creation and complete install-to-launch integration remain incomplete.

Device, network, audio, Bluetooth, files and graphical login components provide domain/UI baselines; production backends and desktop integration remain in development.

## Explicit implementation discrepancies

1. The current kernel/init implementation establishes `luna-init` as the initial userspace task; the remaining gap is completing the architecture-defined bootstrap and child startup of `luna-system-runtime` while keeping `luna-init` as PID 1.
2. Existing builder and runtime paths still contain implementation-era assumptions that must be reconciled with the canonical `LUNA-SYS`/`LUNA-DATA` layout and direct-init model.
3. Semantic boot-success confirmation must be connected to clearing `LunaBootAttempt`.
4. `luna-system-manager` and the bootloader state parser still contain legacy `init` fields for current/factory targets; the current contract intentionally excludes those fields and resolves init through manifests at boot.

These items do not authorize architecture changes.
