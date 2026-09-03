# Project Luna — RFC / ADR Governance

**Status:** development governance

## 1. Purpose

Architecture discussions were historically recorded in several places, which made it possible for obsolete and current decisions to coexist without an obvious boundary. This process makes accepted decisions traceable without turning the large SoT into an unmaintainable change log.

## 2. Source hierarchy

```text
Current accepted architecture
    = docs/ARCHITECTURE.md

Accepted decision ledger
    = docs/decisions/ACCEPTED-DECISIONS.md

Accepted RFCs / ADRs
    = docs/rfc/* and docs/decisions/*

Component contracts
    = docs/architecture/components/*

Historical phase/archive material
    = traceability only
```

If two documents conflict, implementation follows the highest current authority and the conflict must be documented rather than silently ignored.

## 3. RFC numbering

RFC numbers are global and monotonically increasing.

There must be exactly one file for each allocated RFC number. A number must never be reused for a different subject.

Current known RFCs:

- RFC-0001 — Architecture Baseline
- RFC-0002 — Bundle Format v1 (`.lbp`), **Accepted 2026-08-30**

The historical duplicate/obsolete draft of RFC-0002 must be treated as superseded documentation, not as a second active RFC.

## 4. What needs an RFC

Use an RFC for a protocol, file format, externally observable interface, serialization format, or cross-component contract whose exact specification needs independent long-term reference.

Examples:

- Bundle Format;
- System Image format;
- IPC wire protocol;
- durable event format;
- update transaction protocol.

## 5. What needs an ADR

Use an ADR for a concrete architectural choice between alternatives or a decision that establishes ownership/technology without defining a complete wire format.

Examples:

- storage layout;
- logical root strategy;
- namespace ownership;
- session ownership;
- checkpoint backend.

## 6. Component records

Every component receives a component contract document after its responsibility and boundary are accepted.

A component document is not automatically an RFC. It is the implementation-facing contract derived from accepted architecture.

If a component requires a new cross-component contract, that contract receives its own RFC/ADR before implementation is treated as canonical.

## 7. Required RFC/ADR fields

Every new record should contain:

- identifier;
- title;
- status;
- date;
- authority/source;
- problem/context;
- decision;
- ownership;
- dependencies;
- non-goals;
- compatibility/migration implications;
- security implications;
- implementation status;
- explicitly open questions.

## 8. Status transitions

```text
Draft → Proposed → Accepted
                  ↘ Rejected
Accepted → Superseded
```

A draft must not be presented as an implementation requirement.

A superseding decision must explicitly name what it supersedes.

## 9. No silent supersession

Changing code does not change architecture by itself.

When implementation differs from a document, the developer must identify whether:

- code is wrong;
- documentation is stale;
- the decision is intentionally being superseded.

Only the third case changes the architecture, and it requires an explicit accepted record.

## 10. Component development workflow

```text
architecture decision
      ↓
component contract
      ↓
API contract
      ↓
implementation
      ↓
tests
      ↓
integration evidence
```

This permits separate chats to work on a component by supplying its component document plus the small set of referenced contracts instead of the entire historical discussion.
