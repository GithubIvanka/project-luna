# RFC-0002 Bundle Format v1 — Implementation Notes

**Status:** Historical implementation notes / non-normative.

The normative Bundle Format v1 is now defined by `docs/rfc/RFC-0002.md` and is
**Accepted** as of 2026-08-30. This document records implementation guidance and
the transition from the earlier candidate codec.

## Accepted v1 summary

- `.lbp` is the transport/archive representation of a Bundle.
- System Images remain separate SquashFS artifacts.
- Fixed 64-byte `LBP1` header.
- Little-endian integer encoding.
- Fixed 64-byte section entries.
- Exactly one MANIFEST and one PAYLOAD section.
- Optional RESOURCES and SIGNATURE sections.
- BLAKE3-256 integrity/content identity.
- TOML manifest.
- Bundle types: `application`, `component`.
- SemVer `MAJOR.MINOR.PATCH`.
- Deterministic TAR payload.
- ZSTD canonical compression policy.
- Logical mappings only; no physical `DATA/...` paths in manifests.
- Capabilities are requests, not grants.
- No symlinks, hard links or special TAR entries in v1.
- Immutable installed Bundles and independent coexisting versions.
- Optional Ed25519 signature section.
- Signature, trust and permission remain separate.
- Unknown major versions and unsafe/malformed containers reject.
- Delta update transport remains outside `.lbp`.

## Implementation boundary

`luna-bundle` owns Bundle representation, manifest codec, `.lbp` reader/writer,
integrity calculations and format validation.

`luna-app-manager` owns installation, update/removal/import/migration and Bundle
registration.

`luna-security` owns trust/authorization policy.

`luna-root-mapping` owns logical mapping semantics.

`luna-namespace` and runtime own Linux namespace materialization.

## Current implementation status

The repository contains an LBP1 codec with manifest parsing, deterministic payload
construction, section hashing, structural validation and payload extraction.

The codec must remain aligned with the normative header/table definitions in
RFC-0002. In particular, the header is **64 bytes**; an earlier prototype used a
52-byte header and is obsolete.

Signature support is an explicit extension boundary in the first implementation;
cryptographic verification must never be implied merely by the presence of a
SIGNATURE section. Production trust policy belongs to the Security layer.
