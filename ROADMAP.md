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
PC image CI workflow              ← IMPLEMENTED
```

## Phase 2 sequence

### 1. Make the development PC image boot reliably

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

The image contains a musl-native `luna-system-runtime`, early initramfs,
versioned SquashFS System Image, kernel, persistent DATA, and a standard UEFI
fallback at `EFI/BOOT/BOOTX64.EFI`.

Next:

- validate the image in QEMU/OVMF;
- validate a real UEFI machine;
- keep SYSTEM/DATA discovery label based;
- add persistent boot-success state.

See `docs/development/PC-BUILD.md` and `docs/decisions/2026-09-01-PC-BUILD.md`.

### 2. Runtime materialization

The typed runtime contract is:

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

Next:

- resolve runtime to an approved artifact;
- materialize loader/library mappings inside the application namespace;
- version and manage glibc compatibility trees through Luna;
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

The session contract is already:

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

Next, package the final desktop runtime tree into the immutable System Image and keep mutable session/config/state data in DATA.

### 5. Bundle installation → execution

Extend the current LBP1 implementation into a complete development loop:

```text
.lbp
 ↓
verify
 ↓
install into DATA
 ↓
resolve RuntimeSpec
 ↓
security + mapping
 ↓
namespace
 ↓
ApplicationInstance
 ↓
process supervision
```

The final runtime field in RFC-0002 remains a separate Bundle/schema decision; existing Rust callers continue to use `RuntimeKind::Luna` as the compatibility default.

### 6. Durable update / boot state

Connect `luna-system-manager`, `luna-kernel-manager`, `luna-app-manager` and `luna-update-manager` to concrete mutation backends while preserving independent System Image/kernel updates and revision-checked durable state.

### 7. Production hardening

- complete LBP1 conformance and Ed25519 trust binding;
- final IPC/event transport;
- resource controls;
- production-safe child creation instead of complex `pre_exec` namespace setup;
- Secure Boot and release-image signing;
- recovery and interrupted-update validation.

## Non-negotiable constraints

- System Image = direct SquashFS.
- `.lbp` = Bundle transport/archive format.
- SYSTEM is immutable/versioned; DATA is mutable.
- `luna-security` remains the central policy authority.
- `luna-root-mapping` remains the mapping layer.
- `luna-namespace` remains the Linux namespace/materialization layer.
- `luna-system-runtime` is the sole owner of process supervision.
- `UserSession` is the combined user/session entity.
- TTY/serial is development, diagnostic or recovery-only for the normal desktop path.
- Accepted decisions are recorded under `docs/decisions/` and consolidated into the SoT.
