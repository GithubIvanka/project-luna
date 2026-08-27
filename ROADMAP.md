# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated, with Phase 1.6 complete through **1.6-HZ**.

The current stage is the **repository and crate audit**.

## Sequence

```text
Phase 1.6 consolidation
        ↓
Repository / Cargo audit
        ↓
luna-common redesign
        ↓
Concrete crate map
        ↓
Crate/API contracts
        ↓
RFC/specification work where required
        ↓
Prototype
        ↓
Implementation
        ↓
Integration
```

### 1. Repository / Cargo audit — CURRENT

- Compare the actual repository with the Source of Truth.
- Keep only useful current code.
- Remove obsolete implementation assumptions.
- Do not create empty placeholder crates.

**Exit:** repository structure reflects the current architecture.

### 2. `luna-common` audit — COMPLETED

Keep only genuinely cross-cutting primitives. Do not place subsystem-specific errors, runtime state, security policy, filesystem operations or bundle semantics here.

**Result:** the old generic error/result layer was removed; IDs and `Version` remain as small foundational value types.

### 3. Concrete crate map — NEXT

Derive the workspace from the accepted responsibility map. A component may become a library, daemon/service, binary, or a combination where that boundary is real.

**Exit:** every planned crate has a reason to exist, ownership, dependencies and an API boundary.

### 4. API contracts

Define inputs, outputs, state, errors, IPC and persistence before implementation.

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
