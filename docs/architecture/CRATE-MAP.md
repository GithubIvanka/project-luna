# Project Luna — Phase 1.6 Crate Map

**Status:** architecture-driven implementation map
**Source of Truth:** `docs/ARCHITECTURE.md`

This document translates the accepted architecture into concrete Rust package boundaries. It is not a replacement for the architecture and must not introduce new architectural responsibilities.

## Foundation

| Crate | Responsibility | Form |
|---|---|---|
| `luna-common` | Small cross-cutting value types only | lib |
| `luna-fs` | Low-level filesystem abstraction and primitives | lib |
| `luna-root-mapping` | Logical filesystem/path mapping | lib |
| `luna-config` | Configuration model and scoped configuration | lib |

## Policy and management

| Crate | Responsibility | Form |
|---|---|---|
| `luna-security` | Central security/policy authority | lib/backend |
| `luna-app-manager` | Install, update, removal, verification, migrations and package import | lib + bin where required |
| `luna-system-manager` | System state model and queries | lib + bin where required |
| `luna-update-manager` | Executes system/application changes | lib + bin where required |
| `luna-kernel-manager` | Kernel inventory, metadata and compatibility queries | lib + bin where required |
| `luna-device-manager` | Device discovery, volumes and device lifecycle | lib + bin where required |

## Runtime

| Crate | Responsibility | Form |
|---|---|---|
| `luna-system-runtime` | Single system-wide supervision and `UserSession` orchestration | lib + bin where required |
| `luna-user-session` | `UserSession` domain model and lifecycle contract | lib |
| `luna-app-runtime` | `ApplicationInstance` execution/lifecycle boundary | lib + bin where required |

There is no separate `lunad` architecture component and no separate Session Manager. `luna-system-runtime` is the single system-wide runtime/supervisor. `UserSession` is the combined user/session entity.

Runtime ownership is intentionally separate from management ownership. `luna-app-manager` does not own normal application execution.

## Bundle

| Crate | Responsibility | Form |
|---|---|---|
| `luna-bundle` | Internal Bundle domain model, manifest/resource representation and eventual format codec | lib |

The crate exists in the current workspace. `.lbp` remains the transport/archive representation of a Bundle, and RFC-0002 has not yet been accepted as the final wire/archive specification.

## State and events

| Crate | Responsibility | Form |
|---|---|---|
| `luna-state` | Persistent state abstraction, revision and atomic transaction contracts | lib |
| `luna-event` | Event domain, subscriptions and delivery contracts | lib |

The current prototypes are in-memory/contract-level where the durable or OS-backed backend has not yet been implemented.

## User interface

| Crate | Responsibility | Form |
|---|---|---|
| `luna-cli` | Thin CLI client over backend APIs | lib + bin (`luna`) |

A future GUI client is separate and uses the same backend contracts.

## Boot

`luna-boot.efi` is a separate boot-specific project under `boot/luna-boot/`. It is intentionally outside the ordinary userspace workspace because it targets UEFI and operates before the userspace architecture exists.

The current boot implementation has progressed beyond the original scaffold: kernel loading and the test init handoff have been demonstrated through the shell (`sh`). Production trust/signature integration and final boot-compatibility work remain separate tasks.

`luna-boot-state` remains a conceptual architecture boundary and is not yet a separate workspace crate.

## Logging

`luna-log` is not created merely because the name existed historically. A dedicated logging boundary will be introduced when ownership/API requirements justify it.

## Dependency direction

```text
luna-common
    ↑
luna-fs
    ↑
luna-root-mapping

luna-config ───────┐
luna-security ─────┤
luna-state ────────┤
luna-event ────────┤
luna-bundle ───────┤
                   │
management crates ─┤
runtime crates ────┤
luna-cli ──────────┘
```

Higher-level crates consume lower-level contracts. No higher-level crate is allowed to pull application lifecycle, security policy, runtime state, bundle lifecycle or service APIs into `luna-common` or `luna-fs` merely for convenience.

## Current implementation rule

The repository may contain a scaffolded crate before its full backend implementation exists, but the scaffold must represent a responsibility boundary already defined by the architecture.

Before expanding a crate into a real backend, define:

1. responsibility;
2. public API;
3. state ownership;
4. persistence;
5. error model;
6. dependencies;
7. IPC/client boundary where applicable;
8. security boundary.

Existing implementation code is reusable source material, not an authority over the architecture.
