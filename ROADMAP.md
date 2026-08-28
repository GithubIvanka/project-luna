# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated, with Phase 1.6 complete through **1.6-HZ**.

The repository audit and architecture-driven crate map are complete. The first foundation/domain API pass is also complete.

## Sequence

```text
Phase 1.6 consolidation
        ↓
Repository / Cargo audit
        ↓
Concrete crate map
        ↓
Foundation/domain API contracts  ← COMPLETED
        ↓
Manager/runtime API contracts     ← CURRENT
        ↓
RFC/specification work where required
        ↓
Prototype
        ↓
Implementation
        ↓
Integration
```

### 1. Repository / Cargo audit — COMPLETED

The actual `main` branch was audited and aligned with the accepted architecture. Historical empty crates are not treated as implementation commitments.

### 2. `luna-common` audit — COMPLETED

`luna-common` now contains only small shared value types. Subsystem-specific errors, runtime state, security policy, filesystem operations and bundle semantics remain outside the crate.

### 3. Concrete crate map — COMPLETED / SCAFFOLDED

The workspace is architecture-driven and includes the defined foundation, mapping, configuration, security, state/event, bundle-domain, management, runtime, session and CLI boundaries. Bootloader and logging remain separately gated.

See [`docs/architecture/CRATE-MAP.md`](docs/architecture/CRATE-MAP.md).

### 4. Foundation/domain API contracts — COMPLETED

The first explicit API pass covers:

- `luna-common` value types;
- `luna-fs` low-level filesystem operations;
- `luna-root-mapping` per-namespace exact-file mapping;
- `luna-config` layered configuration;
- `luna-security` policy authority;
- `luna-state` durable state boundary;
- `luna-event` event contracts;
- `luna-bundle` internal bundle domain model;
- `luna-user-session` lifecycle model.

### 5. Manager/runtime API contracts — CURRENT

Next define concrete public contracts for:

- `luna-system-manager`;
- `luna-kernel-manager`;
- `luna-update-manager`;
- `luna-app-manager`;
- `luna-device-manager`;
- `luna-system-runtime`;
- `luna-app-runtime`;
- `luna-cli`.

Their APIs must consume the established lower-level contracts rather than moving higher-level responsibilities downward.

### 6. RFC/specification work

`RFC-0001` formalizes the architectural baseline.

`RFC-0002` separately designs Bundle Format v1 (`.lbp`). It must not be accepted from an earlier proposal without review.

System Image, kernel and boot specifications remain separate from Bundle Format.

### 7. Prototype

Prototype the highest-risk boundaries first, especially logical-root/materialization, bundle parsing/validation, resource control, and boot compatibility where appropriate.

### 8. Implementation

Implement one component at a time under the Source of Truth and its API contract.

### 9. Integration

Validate end-to-end behaviour against the canonical runtime, storage, security, state and recovery models.

## Non-negotiable constraints

- System Image = direct SquashFS, not `.lbp`.
- `.lbp` is a separate Bundle Format/transport/archive format.
- SYSTEM and DATA remain separate.
- `luna-app-manager` does not own normal application execution.
- `luna-security` remains the policy authority.
- `luna-root-mapping` remains a narrow mapping layer.
- `luna-fs` remains low-level filesystem abstraction.
- One `luna-system-runtime` coordinates multiple `UserSession`s.
- Linux namespaces/resource controls are implementation mechanisms, not the architecture itself.
- Accepted decisions are not silently changed.
