# Project Luna — RFC Index

**Status:** canonical RFC navigation/numbering ledger
**Authority:** `docs/development/RFC-ADR-PROCESS.md`

## Allocated RFCs

| RFC | Title | Status | Canonical file |
|---|---|---|---|
| 0001 | Architecture Baseline | Accepted | `RFC-0001.md` |
| 0002 | Bundle Format v1 (`.lbp`) | Accepted, 2026-08-30 | `RFC-0002.md` |

There is currently **one RFC-0002**. The file `docs/decisions/0007-rfc-0002-bundle-format-v1.md` is an ADR recording acceptance of RFC-0002, not a second RFC. `RFC-0002-BUNDLE-FORMAT-V1-IMPLEMENTATION-NOTES.md` is non-normative implementation history.

## Planned specifications

The following subjects are important enough to receive dedicated RFC/ADR treatment when their contracts are actually discussed and accepted. They are intentionally **not assigned numbers yet**:

- System Image format and manifest;
- kernel/image compatibility metadata;
- Luna IPC transport/wire contract;
- durable event transport/serialization;
- update transaction protocol;
- authentication/session IPC;
- application permission/confirmation protocol;
- external volume/device protocol where a stable wire contract is required.

Do not reserve a number by inventing a specification. Allocate the next number only when the subject is ready for a real RFC.

## Component documents

Every existing architectural component has a contract document under `docs/architecture/components/`. Those documents are implementation contracts, not fake RFCs. A component receives an RFC/ADR only when a separate protocol/format/architectural decision actually requires one.
