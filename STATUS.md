# Project Luna — Status

Last updated: 2026-08-26

> This document reflects the current state of Project Luna.
> The architectural Source of Truth is [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
> This file is a status snapshot, not an architectural authority.

---

## Overall state

Project Luna is in the **design-first phase**. The architecture is consolidated
through **Phase 1.5**. The Rust workspace exists and builds, but no major
subsystem (bootloader, runtime, bundle format implementation) is implemented yet.

Current focus: formalizing specifications — **RFC-0001** (Architecture baseline)
and **RFC-0002** (Bundle Format v1).

---

## Phase status

| Phase | Scope | Status |
|-------|-------|--------|
| Phase 1.1 | Initial architecture baseline | ✅ Accepted and consolidated |
| Phase 1.2 | Storage model, checkpoints, CLI direction | ✅ Accepted and consolidated |
| Phase 1.3 | Component responsibility map | ✅ Accepted and consolidated |
| Phase 1.4 | Identity, trust, authority, operations, state, resources | ✅ Accepted and consolidated |
| Phase 1.5 | Security, Root Mapping, Runtime contracts | ✅ Accepted and consolidated |
| Next phase | Repository/crate architecture → concrete Rust APIs | 🔜 Planned |

Per section 108.19: Phases 1.1–1.5 are accepted and consolidated into
`docs/ARCHITECTURE.md`. Phase 1.5 establishes the accepted contracts between
Security, Root Mapping, Application Runtime, System Runtime, UserSession, IPC,
Events/Operations and Diagnostics/Health.

---

## What exists in code

### Rust workspace (builds successfully)

```
project-luna/
├── Cargo.toml
├── components/
│   ├── luna/           # main CLI entry point
│   ├── luna-bundle/    # bundle format representation
│   ├── luna-common/    # shared fundamental types
│   ├── luna-config/    # configuration
│   ├── luna-fs/        # filesystem abstraction
│   └── luna-log/       # logging
└── rust-toolchain.toml
```

`cargo build` completes successfully (`Finished dev profile`).

### What this means

- ✅ The base Rust workspace exists and compiles.
- ❌ This does **not** mean the bootloader, Bundle Format, System Image,
  kernel manager, runtime, or device manager are implemented.

### Architectural components not yet created as crates

Per the repository rule (section 51.1): a subsystem appears in the repository
**only when its real development starts**. The following are architectural
subsystems, not yet repository crates:

- `luna-boot` (luna-boot.efi)
- `luna-system-manager`
- `luna-app-manager`
- `luna-app-runtime`
- `luna-system-runtime`
- `luna-device-manager`
- `luna-update-manager`
- `luna-kernel-manager`
- `luna-root-mapping`
- `luna-security`

> ⚠️ Component counts are illustrative. The authoritative component map is
> section 105.23 of ARCHITECTURE.md.

---

## Accepted architecture (summary)

Full details in ARCHITECTURE.md. Key accepted invariants:

### Storage model (105.1)
- Four physical areas: `EFI`, `SYSTEM`, `DATA`, `SWAP`.
- DATA layout:
  ```
  DATA/
  ├── system/
  │   ├── apps/
  │   ├── drivers/
  │   ├── libs/
  │   ├── volumes/
  │   └── config/
  ├── users/<user>/{home, data, config}/
  └── cache/
  ```

### Boot (105.16)
- Custom `luna-boot.efi` bootloader.
- Key `B` opens Boot Menu.
- Compatibility-aware fallback; soft fallback for image failure.
- Boot state is event-driven, not rewritten on every boot.

### System Images & kernels (105.14)
- System Image = direct SquashFS (`*.squashfs`) + per-image manifest (`*.toml`).
- System Images and kernels are independent and versioned separately.
- `current` = active image/kernel combination; `factory` = immutable original.

### Applications & runtime (105.8, 105.9, 109.6)
- Applications are immutable bundles (`.lbp` is transport/archive format).
- Launch chain: `luna-system-runtime → UserSession → luna-app-runtime → ApplicationInstance`.
- Each `ApplicationInstance` gets its own mount/filesystem namespace.

### Security (105.7, 108.5, 109.1)
- `luna-security` is the central policy authority.
- No permanent root user; no `sudo`/`su` privilege hierarchy.
- `Subject + Resource + Action + Context → PolicyDecision`.
- Integrity / Authenticity / Trust / Authorization are separate concepts.

### Health model (108.12, 109.9)
- `Healthy / Degraded / Recovering / Failed / Emergency`.
- `Emergency` is a health state, not a Boot Menu mode.

---

## What is explicitly NOT yet defined

Per section 68, the following must not be treated as finalized specifications:

- exact `.lbp` binary format / structure
- exact TOML Bundle manifest
- exact TOML System Image manifest
- exact `current` / `factory` format
- exact kernel metadata structure
- exact kernel-panic detection procedure
- exact soft fallback procedure
- exact SquashFS hybrid loading implementation
- exact application permission model
- exact device automount backend
- exact service backend (OpenRC-like direction only)
- final `luna` CLI command set
- exact runtime namespace structure
- bundle/image signatures & cryptographic verification policy
- update transaction protocol

If a chat discusses these, it must **design** them, not assume they exist.

---

## Next steps

### Immediate
1. **RFC-0001** — Architecture baseline (fundament: purpose, principles, disk
   layout, boot architecture, system/data separation, immutable model,
   versioned images, kernel separation).
2. **RFC-0002** — Bundle Format v1 (`.lbp`).

### After RFCs
3. **System Image Specification** — SquashFS + per-image manifest + versioning
   + compatibility + retention.
4. **Boot Specification** — luna-boot.efi behaviour, boot state, fallback, factory.
5. **Kernel Specification** — metadata, compatibility, selection.
6. **Repository/crate architecture** — translate Phase 1.5 contracts into
   concrete Rust workspace boundaries and APIs.
7. **Prototype** — only after specifications are precise enough.

Development order (section 98):
```
Architecture → RFC → Format → Interfaces → Prototype → Implementation → Integration
```

---

## Risks / watchpoints

1. **Design phase duration** — avoid indefinite specification without prototyping.
2. **Hybrid SquashFS loading** — technically non-trivial; keep it a spec task,
   don't over-commit to one kernel mechanism early (109.4).
3. **UEFI bootloader** — `luna-boot.efi` runs in a very different environment
   than Linux userspace; do not assume `luna-log` etc. reuse directly (54).
4. **`luna-common` hygiene** — must not become a dumping ground (53, 105.23).
5. **Don't confuse "discussed" with "accepted"** (103, 105.28).

---

## Notes for maintainers

- Metrics like exact LOC, test coverage, and decision counts are **not tracked
  in the Source of Truth** and are intentionally omitted here to avoid inventing
  numbers. Add them only when actually measured.
- When a phase closes, consolidate its accepted decisions into
  `docs/ARCHITECTURE.md` and update this status file.
```
