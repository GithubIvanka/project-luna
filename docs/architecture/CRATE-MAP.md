# Project Luna — Current Crate Map

**Status:** current implementation map  
**Source of Truth:** `docs/ARCHITECTURE.md`

This document describes the current Rust package boundaries. It does not define architecture independently.

## Foundation

| Crate | Responsibility | Form |
|---|---|---|
| `luna-common` | Small shared value types and identifiers | lib |
| `luna-fs` | Low-level filesystem primitives and metadata | lib |
| `luna-root-mapping` | Logical path and mapping semantics | lib |
| `luna-namespace` | Linux namespace/materialization primitives | lib |
| `luna-config` | Configuration model and scoped configuration | lib |

## Policy and state

| Crate | Responsibility | Form |
|---|---|---|
| `luna-security` | Policy, authorization, grants and trust | lib |
| `luna-state` | Persistent state domain, storage abstraction and transactions | lib |
| `luna-event` | Event domain, subscriptions and delivery contracts | lib |

## Bundle and management

| Crate | Responsibility | Form |
|---|---|---|
| `luna-bundle` | Bundle domain, manifest, validation and accepted RFC-0002/LBP1 codec | lib |
| `luna-app-manager` | Application install/import/update/removal/verification/migration | lib + bin where required |
| `luna-system-manager` | System state model and queries | lib + bin where required |
| `luna-update-manager` | State-changing update execution, checkpoints and rollback coordination | lib + bin where required |
| `luna-kernel-manager` | Kernel inventory, metadata and compatibility queries | lib + bin where required |
| `luna-device-manager` | Device and volume discovery/lifecycle | lib + bin where required |

## Runtime

| Crate | Responsibility | Form |
|---|---|---|
| `luna-system-runtime` | Single system-wide runtime/supervision and UserSession orchestration | lib + bin where required |
| `luna-user-session` | Combined UserSession domain and lifecycle contract | lib |
| `luna-app-runtime` | ApplicationInstance execution/lifecycle and execution-environment preparation | lib + bin where required |

There is no separate `lunad` or Session Manager architecture component. `luna-system-runtime` is the system-wide runtime/supervisor.

## CLI

| Crate | Responsibility | Form |
|---|---|---|
| `luna-cli` | Thin user-facing CLI client over backend operations | lib + bin |

GUI is a separate thin client using the same backend contracts.

## Boot

`luna-boot.efi` is a separate UEFI project under `boot/luna-boot/` and is intentionally outside the ordinary userspace workspace.

The current boot project includes UEFI/Linux boot protocol handling and test infrastructure. Its dedicated development path has demonstrated kernel loading through the test init to `sh`.

## Dependency direction

```text
luna-common
    ↑
luna-fs
    ↑
luna-root-mapping
    ↑
luna-namespace

luna-config   ─────┐
luna-security ─────┤
luna-state    ─────┤
luna-event    ─────┤
luna-bundle   ─────┤
                   │
management crates ─┤
runtime crates   ──┤
luna-cli        ──┘
```

Higher-level components consume lower-level contracts. Domain ownership remains explicit.

## Current status

The current workspace contains all crates above because each corresponds to an architecture-defined boundary that is already under active implementation or contract/integration hardening. A crate must not absorb unrelated responsibilities merely for convenience.

RFC-0002 Bundle Format v1 is accepted. `luna-bundle` contains the current LBP1 implementation.

The first durable `luna-state` backend is `redb`.

`luna-namespace` contains the first Linux namespace/materialization backend.

`luna-boot.efi` remains a separate boot boundary.
