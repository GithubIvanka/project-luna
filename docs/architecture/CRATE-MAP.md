# Project Luna — Phase 1.6 Crate Map

**Status:** implementation scaffold / API design pending
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
| `luna-app-manager` | Install, update, removal, verification, migrations and package import | lib + bin |
| `luna-system-manager` | System state model and queries | lib + bin |
| `luna-update-manager` | Executes system/application changes | lib + bin |
| `luna-kernel-manager` | Kernel inventory, metadata and compatibility queries | lib + bin |
| `luna-device-manager` | Device discovery, volumes and device lifecycle | lib + bin |

## Runtime

| Crate | Responsibility | Form |
|---|---|---|
| `luna-system-runtime` | System-wide supervision and UserSession orchestration | lib + bin |
| `luna-app-runtime` | ApplicationInstance execution boundary | lib + bin |

Runtime ownership is intentionally separate from management ownership. `luna-app-manager` does not own normal application execution.

## User interface

| Crate | Responsibility | Form |
|---|---|---|
| `luna-cli` | Thin CLI client over backend APIs | lib + bin (`luna`) |

GUI is not forced into the CLI crate. A future GUI client may use the same backend contracts.

## Deliberately not scaffolded yet

### `luna-bundle`

Not created as an implementation crate yet. `.lbp` and Bundle Format v1 remain subject to RFC-0002 design. The existing architecture explicitly says not to accept an earlier `.lbp` proposal automatically.

### `luna-boot.efi`

Not part of the ordinary userspace workspace scaffold. Bootloader implementation requires its own target/toolchain/API boundary and remains a separate implementation task.

### `luna-log`

Not created merely as a historical name. Logging requirements will be introduced when an owning boundary requires them.

## Dependency direction

The initial dependency direction is deliberately shallow:

```text
luna-common
    ↑
luna-fs
    ↑
luna-root-mapping

luna-config ───────┐
luna-security ─────┤
                   │
management crates ─┤
runtime crates ────┤
luna-cli ──────────┘
```

No higher-level crate is allowed to pull application lifecycle, security policy, runtime state or bundle semantics into `luna-common` or `luna-fs`.

## Scaffold rule

The current crates are architectural scaffolds, not completed implementations. Empty public structs are placeholders for the ownership boundary only; they are not final APIs.

Before implementation of a crate, define:

1. responsibility;
2. public API;
3. state ownership;
4. persistence;
5. error model;
6. dependencies;
7. IPC/client boundary where applicable;
8. security boundary.
