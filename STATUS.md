# Project Luna — Status

Last updated: 2026-09-01

> `docs/ARCHITECTURE.md` is the architectural Source of Truth. Accepted architecture decisions through Phase 1.6-HZ and subsequent accepted decisions are consolidated there. Dated decision records under `docs/decisions/` preserve implementation history and boundaries.

## Overall state

Project Luna has completed the architecture decision cycle through **Phase 1.1–1.6-HZ** and is in **Phase 2 runtime/boot integration, PC bring-up, desktop integration and hardening**.

The Phase 2 stacked integration chain was consolidated into `main` as commit `9013a8efb305c1b98587b0fc356cf79e473dcd39`. New work continues from the single `development` branch.

### Phase status

| Area | Status |
|---|---|
| Architecture 1.1–1.6-HZ | Accepted and consolidated |
| Post-1.6 accepted architecture decisions | Consolidated into the Source of Truth |
| Foundation/domain APIs | Implemented baseline |
| Runtime hierarchy | `luna-system-runtime → UserSession → luna-app-runtime → ApplicationInstance` |
| Typed runtime contract | `RuntimeKind` / `RuntimeSpec` implemented as ApplicationInstance execution properties |
| Generic `luna-runtime` crate | Explicitly rejected and removed |
| Runtime ↔ mapping ↔ security | Contract implemented; authorization precedes namespace materialization |
| Linux namespace/materialization | Development mount namespace backend implemented; production child-creation hardening remains |
| Persistent state | Durable redb backend implemented under `DATA/system/state` |
| Update/checkpoint/rollback | Durable orchestration implemented; concrete mutation backends remain |
| Bundle Format v1 | **RFC-0002 Accepted (2026-08-30); LBP1 codec under conformance/security hardening** |
| `luna-system-runtime` | Real child supervision and UserSession lifecycle ownership implemented |
| `luna-app-runtime` | ApplicationInstance lifecycle and execution setup implemented |
| `luna-boot.efi` | GUI splash; dynamic SYSTEM image/kernel discovery; manifest validation; compatible kernel selection; soft fallback; ordered Boot Menu |
| Recovery / Factory boot | Metadata-role discovery and target execution implemented; final recovery tooling/repair UX remains |
| External/USB boot | UEFI `EFI/BOOT/BOOTX64.EFI` chainload development backend implemented |
| x86_64 PC image | Reproducible GPT/UEFI development image builder implemented |
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

Pressing `B` opens the exceptional text Boot Menu. The fixed action order is:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

Verbose Boot suppresses the GUI splash and enables full kernel diagnostics for that boot. TTY/serial remains a development, diagnostic or recovery mechanism only.

## Boot discovery

`luna-boot.efi` discovers normal boot targets from SYSTEM instead of using a hardcoded release list:

```text
SYSTEM/images/*.squashfs + adjacent *.toml
              ↓
       manifest validation
              ↓
SYSTEM/kernels/<version>/bzImage
              ↓
   compatible kernel filter
              ↓
      version ordering
              ↓
       BootTarget catalog
```

Recovery and Factory images are special System Image roles and remain outside the normal selection list. External boot is a UEFI-only chainload path and does not depend on Linux or the internal System Image being healthy.

## Runtime architecture

The ownership hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

The execution pipeline is separate:

```text
ApplicationInstance
    ↓
resource/declaration request
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
luna-namespace
    ↓
process execution
    ↓
luna-system-runtime supervision
```

`RuntimeKind` is only a typed property of the ApplicationInstance execution environment:

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved glibc compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

There is no generic `luna-runtime` crate or daemon.

## Current implementation sequence

1. Validate the complete PC image in QEMU/OVMF and on real UEFI hardware.
2. Package the final graphical `luna-login` + niri + Noctalia desktop runtime tree.
3. Extend `luna-app-runtime` with runtime-specific loader/library mappings while preserving security and mapping ownership.
4. Finish fine-grained Security-to-mapping/device authorization and filtered `/dev` population.
5. Complete durable boot/update state integration and persistent boot-success state.
6. Finish LBP1 conformance and Ed25519 verification/trust binding.
7. Implement IPC/event transport, resource enforcement and device/volume integration.
8. Complete real `.lbp` installation and ApplicationInstance launch/recovery.
9. Replace prototype `pre_exec` namespace setup with a production-safe child-creation primitive.

## Decision records

```text
docs/decisions/2026-09-01-RUNTIME-INTEGRATION.md
docs/decisions/2026-09-01-RUNTIME-CONTRACT.md
docs/decisions/2026-09-01-PC-BUILD.md
docs/decisions/2026-09-01-GRAPHICAL-BOOT-SESSION.md
docs/decisions/2026-09-01-BOOT-DISCOVERY.md
docs/decisions/2026-09-01-GIT-WORKFLOW.md
docs/decisions/2026-09-01-MAIN-PROTECTION.md
docs/decisions/2026-09-01-GRAPHICAL-BOOT-SESSION.md
```
