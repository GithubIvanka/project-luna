# Project Luna — Architecture Component Documentation

**Status:** canonical component-documentation index
**Authority:** `docs/ARCHITECTURE.md`
**Purpose:** provide a small, stable document for each architectural boundary so development can happen component-by-component without reconstructing the whole SoT.

## Authority model

```text
docs/ARCHITECTURE.md
        ↓
accepted decisions / RFCs
        ↓
docs/architecture/components/*
        ↓
implementation
```

A component document describes the current contract. It does not silently create a new architecture. If a document conflicts with `docs/ARCHITECTURE.md`, the conflict is a documentation defect that must be resolved before implementation continues.

Historical phase documents and superseded ADR material remain traceability only.

## Component documentation

### Physical / boot architecture

- `DISK-LAYOUT.md` — EFI / SYSTEM / DATA / SWAP, filesystems, ownership and on-disk layout.
- `LUNA-BOOT.md` — UEFI loader, boot selection, compatibility, fallback and menu contract.
- `SYSTEM-IMAGE.md` — SquashFS System Image, manifest, versioning, kernel compatibility and retention.
- `RECOVERY-FACTORY.md` — Recovery and Factory environments and their boundaries.

### Core userspace boundaries

- `LUNA-COMMON.md`
- `LUNA-FS.md`
- `LUNA-ROOT-MAPPING.md`
- `LUNA-NAMESPACE.md`
- `LUNA-CONFIG.md`
- `LUNA-SECURITY.md`
- `LUNA-STATE.md`
- `LUNA-EVENT.md`
- `LUNA-BUNDLE.md`

### Management

- `LUNA-APP-MANAGER.md`
- `LUNA-SYSTEM-MANAGER.md`
- `LUNA-UPDATE-MANAGER.md`
- `LUNA-KERNEL-MANAGER.md`
- `LUNA-DEVICE-MANAGER.md`

### Runtime / session / login

- `LUNA-SYSTEM-RUNTIME.md`
- `USER-SESSION.md`
- `LUNA-APP-RUNTIME.md`
- `LUNA-LOGIN.md`

### User-facing / hardware service boundaries

- `LUNA-CLI.md`
- `LUNA-FILES.md`
- `LUNA-AUDIO.md`
- `LUNA-NETWORK.md`
- `LUNA-BLUETOOTH.md`

## Cross-cutting rules

1. No component may be invented because an implementation is inconvenient.
2. A Linux utility, daemon or helper is not automatically a Luna architectural component.
3. `UserSession` is the session boundary. There is no separate `luna-session` or `luna-run-session` component.
4. `luna-system-runtime` is the single system-wide runtime/supervisor and coordinates UserSessions.
5. `luna-app-runtime` owns ApplicationInstance execution and lifecycle.
6. Managers own domain state-changing operations; update-manager owns update transaction execution.
7. `luna-security` owns authorization policy. Mapping and filesystem layers do not grant permissions.
8. `luna-boot.efi` is a separate UEFI boundary, outside the ordinary userspace workspace.
9. System Image is SquashFS. `.lbp` is a different format and must never be substituted for a System Image.
10. New architectural boundaries require an accepted decision before they become part of the component map.

## Status vocabulary

- **Accepted** — architecture/contract is explicitly accepted.
- **Implemented** — repository contains a meaningful implementation of the accepted contract.
- **Integration** — implementation exists but integration is incomplete.
- **Planned** — accepted direction, not yet implemented.
- **Open** — decision is not fixed; implementation must not guess it.

A component document must distinguish these states instead of presenting planned behavior as implemented behavior.
