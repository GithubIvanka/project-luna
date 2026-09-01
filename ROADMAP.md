# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes implementation sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6-HZ** are accepted/consolidated. RFC-0002 Bundle Format v1 was accepted on **2026-08-30**. The project is now in **Phase 2 runtime integration, PC bring-up, desktop integration and hardening**.

## Completed foundation

```text
Architecture / SoT                ← COMPLETED
Repository / Cargo audit          ← COMPLETED
Domain + manager API baseline     ← COMPLETED
Logical mapping backend           ← IMPLEMENTED
Linux namespace backend           ← IMPLEMENTED
Persistent redb state             ← IMPLEMENTED
Update/checkpoint engine          ← IMPLEMENTED
RFC-0002 / LBP1                   ← ACCEPTED / HARDENING
System runtime supervisor         ← IMPLEMENTED
UserSession graphical lifecycle   ← IMPLEMENTED
Typed runtime contract            ← IMPLEMENTED
Runtime ↔ mapping ↔ Security      ← IMPLEMENTED CONTRACT
QEMU userspace bring-up           ← IMPLEMENTED DEVELOPMENT PATH
x86_64 PC image builder           ← IMPLEMENTED DEVELOPMENT PATH
Guarded PC installer              ← IMPLEMENTED DEVELOPMENT PATH
PC image CI workflow               ← IMPLEMENTED
Graphical boot splash             ← IMPLEMENTED DEVELOPMENT PATH
Boot Menu full action set         ← IMPLEMENTED DEVELOPMENT PATH
```

## Phase 2 sequence

### 1. Make the graphical development PC image boot reliably

Current artifact:

```text
dist/luna-pc.img
```

with:

```text
EFI     128 MiB
SYSTEM  384 MiB
DATA    512 MiB
```

The image uses a musl-native `luna-system-runtime`, early initramfs,
versioned SquashFS System Image, kernel, persistent DATA, and a standard UEFI
fallback at `EFI/BOOT/BOOTX64.EFI`.

Normal boot is graphical; the image builder requires a prepared graphical root
containing the login surface and `niri-session` rather than producing a TTY or
shell fallback.

Next:

- validate the image in QEMU/OVMF;
- validate a real UEFI machine;
- keep SYSTEM/DATA discovery label based;
- add persistent boot-success state.

### 2. Runtime materialization

The typed runtime contract is:

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

`RuntimeKind` is only a typed execution-environment value. There is no generic
runtime resolver crate. `luna-app-runtime` owns application launch semantics,
while `luna-root-mapping`, `luna-security`, and `luna-namespace` provide mapping,
authorization, and namespace materialization contracts.

Next:

- resolve runtime-specific loader/library mappings inside `luna-app-runtime`;
- version and manage glibc compatibility trees through the existing application/runtime boundaries;
- reject libc mixing within one process;
- keep physical runtime paths hidden behind mapping.

### 3. Security and device boundary

Complete the enforcement layer around the existing runtime path:

- fine-grained mapping authorization;
- filtered `/dev` population;
- secure physical-path and symlink validation;
- resource enforcement before execution;
- device authorization and volume integration.

### 4. Real graphical System Image

The user-facing boot/session contract is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
GUI boot splash
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

There is no normal TTY login and no shell fallback. Pressing `B` enters the
exceptional Boot Menu, which provides the accepted controls:

```text
Continue to Luna
System Image selection
Recovery Environment
Factory Environment
Boot from USB / External Device
Verbose Boot
```

Verbose Boot suppresses the graphical splash and exposes full kernel
diagnostics for that boot.

Next, package the final desktop runtime tree into the immutable System Image,
including the login surface, niri, Noctalia and required device/portal support,
while keeping mutable session/config/state data in DATA.

### 5. Bundle installation → execution

Extend the current LBP1 implementation into a complete development loop:

```text
.lbp
 ↓
verify
 ↓
install into DATA
 ↓
RuntimeKind
 ↓
luna-app-runtime
 ↓
security + mapping
 ↓
namespace
 ↓
ApplicationInstance
 ↓
process supervision
```

The final runtime field in RFC-0002 remains a separate Bundle/schema decision;
existing Rust callers continue to use `RuntimeKind::Luna` as the compatibility
default.

### 6. Durable update / boot state

Connect `luna-system-manager`, `luna-kernel-manager`, `luna-app-manager` and
`luna-update-manager` to concrete mutation backends while preserving independent
System Image/kernel updates and revision-checked durable state.

### 7. Production hardening

- complete LBP1 conformance and Ed25519 trust binding;
- final IPC/event transport;
- resource controls;
- production-safe child creation instead of complex `pre_exec` namespace setup;
- Secure Boot and release-image signing;
- recovery and interrupted-update validation.

## Git workflow

`main` is the canonical integration branch and is protected by the repository
ruleset. Normal implementation work uses one short-lived development branch
from current `main` and one PR against `main`. Stacked `integration/*` PR chains
are not the normal workflow. See `docs/decisions/2026-09-01-GIT-WORKFLOW.md` and
`docs/decisions/2026-09-01-MAIN-PROTECTION.md`.

## Non-negotiable constraints

- System Image = direct SquashFS.
- `.lbp` = Bundle transport/archive format.
- SYSTEM is immutable/versioned; DATA is mutable.
- `luna-security` remains the central policy authority.
- `luna-root-mapping` remains the mapping layer.
- `luna-namespace` remains the Linux namespace/materialization layer.
- `luna-system-runtime` is the sole owner of process supervision.
- `UserSession` is the combined user/session entity.
- TTY/serial is development, diagnostic or recovery-only; it is never the normal user path.
- Normal boot uses a GUI splash, graphical login and the Wayland → niri → Noctalia desktop path.
- Boot Menu is entered only on explicit request and contains the accepted System Image, Recovery, Factory, External/USB and Verbose Boot controls.
- Verbose Boot suppresses the splash and enables full diagnostics for that boot.
- No generic `luna-runtime` component is part of the architecture.
- Accepted decisions are recorded under `docs/decisions/` and consolidated into the SoT.
