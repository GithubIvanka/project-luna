# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated, with Phase 1.6 complete through **1.6-HZ**.

The repository audit is complete and the architecture-driven crate map has been scaffolded.

## Sequence

```text
Phase 1.6 consolidation
        ↓
Repository / Cargo audit
        ↓
Concrete crate map
        ↓
Crate/API contracts              ← CURRENT
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

The actual `main` branch was reduced to useful code and then rebuilt around the accepted architecture. Historical empty crates are not treated as implementation commitments.

### 2. `luna-common` audit — COMPLETED

Keep only genuinely cross-cutting primitives. Subsystem-specific errors, runtime state, security policy, filesystem operations and bundle semantics do not belong here.

### 3. Concrete crate map — COMPLETED / SCAFFOLDED

The current workspace is architecture-driven and includes the foundation, mapping, configuration, security, management, runtime and CLI boundaries documented in `docs/architecture/CRATE-MAP.md`.

Scaffolding does not mean the APIs are final.

### 4. API contracts — CURRENT

For each crate define:

- public API;
- ownership and state;
- persistence;
- errors;
- dependencies;
- IPC/client boundary;
- security boundary;
- async requirements.

### 5. RFC/specification work

`RFC-0001` formalizes the architectural baseline.

`RFC-0002` separately designs Bundle Format v1 (`.lbp`). It must not be accepted from an earlier proposal without review.

System Image, kernel and boot specifications remain separate from Bundle Format.

### 6. Prototype

Prototype the highest-risk boundaries first, especially logical-root/materialization, bundle parsing/validation, and boot compatibility where appropriate.

### 7. Implementation

Implement one component at a time under the Source of Truth and its API contract.

### 8. Integration

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
