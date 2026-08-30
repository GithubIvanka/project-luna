# Decision 0007 — Bundle Format v1 (`.lbp`)

**Status:** ACCEPTED  
**Date:** 2026-08-30  
**Related RFC:** RFC-0002

## Decision

Project Luna accepts RFC-0002, Bundle Format v1 (`.lbp`). The normative
specification is `docs/rfc/RFC-0002.md`.

## Accepted format

- Fixed 64-byte `LBP1` header.
- Little-endian integer encoding.
- Fixed 64-byte section entries.
- Exactly one `MANIFEST` and one `PAYLOAD` section.
- Optional `RESOURCES` and `SIGNATURE` sections.
- BLAKE3-256 for section integrity and Bundle ContentIdentity.
- TOML manifest.
- Bundle types: `application` and `component`.
- SemVer `MAJOR.MINOR.PATCH`.
- Deterministic TAR payload.
- ZSTD canonical compression policy.
- Logical mappings are declared in the manifest and never contain physical
  `DATA/...` paths.
- Mapping declarations and capability declarations are requests, not
  authorization grants.
- v1 payloads exclude symlinks, hard links and special filesystem entries.
- Installed Bundle payloads are immutable.
- Different application versions may coexist independently.
- Optional Ed25519 signature section.
- Signature validity, trust and authorization remain separate concepts.
- Unknown major format versions and malformed/unsafe containers fail closed.
- Delta update transport is outside RFC-0002.

## Ownership boundaries

`luna-bundle` owns the Bundle representation and `.lbp` format codec.

`luna-app-manager` owns install, update, removal, import and migration procedures.

`luna-security` owns trust and authorization policy.

`luna-root-mapping` owns logical mapping semantics.

`luna-namespace` and runtime own Linux namespace materialization.

## Historical note

`docs/rfc/RFC-0002-V1-DECISION-SET.md` is retained as the pre-acceptance candidate
record and is marked `SUPERSEDED`. Any conflicting older proposal is subordinate
to the accepted RFC.
