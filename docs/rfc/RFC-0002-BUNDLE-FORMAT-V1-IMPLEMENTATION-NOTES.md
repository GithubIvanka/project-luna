# RFC-0002 — Bundle Format v1 implementation notes

**Status:** Draft companion note; RFC-0002 itself remains **Draft / Proposal**.

This note records the implementation boundary reached during the Phase 1.6 integration pass. It does not accept the wire format and does not override `docs/rfc/RFC-0002.md`.

## 1. Accepted implementation boundary

The current repository treats a bundle as a domain object with:

- `BundleMetadata` (identity, version, kind);
- `BundleResource` (logical path + bundle-relative source path);
- `BundleManifest`;
- structural manifest validation.

The runtime validates that logical resource paths are absolute, contain no parent traversal, and are unique. Bundle source paths are bundle-relative and cannot escape the bundle through absolute paths or `..` components.

## 2. `.lbp` remains transport-only

The `.lbp` archive/container is not implemented as a binary parser in the domain crate. The domain model is deliberately independent from the transport representation.

The current candidate is recorded separately in `docs/rfc/RFC-0002-V1-DECISION-SET.md`. It proposes the existing structured `LBP1` container with a fixed little-endian header/section table, BLAKE3-256 integrity, TOML manifest, deterministic tar semantics with zstd payload compression, and an optional Ed25519 signature section. These are candidate decisions, not accepted architecture, until the acceptance gate is completed.

## 3. Mapping boundary

A validated bundle resource is converted by the application/runtime layer into a `LogicalPath` and resolved through `luna-root-mapping`.

The bundle layer does not own physical `DATA` paths, authorization, mounts, or namespace creation.

## 4. Security boundary

Capability declarations in a future bundle manifest remain requests only. Authorization is evaluated by `luna-security` through `PolicyAuthority` and is not inferred from the presence of a bundle resource or mapping.

A deterministic `StaticPolicyAuthority` now exists for contract tests and early prototypes. It is not the final persisted policy backend.

## 5. Runtime boundary

`luna-app-runtime` now has an in-memory lifecycle prototype that validates bundle structure and mappings, requires an active `UserSession`, creates an `ApplicationInstance`, supports stop/failure transitions, and exposes an explicit namespace-preparation boundary backed by `luna-namespace`.

Namespace preparation validates an existing application instance and asks the Linux backend to materialize the logical root. It deliberately does not perform the final process `exec` or move application lifecycle ownership into `luna-namespace`.

## 6. Candidate RFC decisions

The companion decision set records one coherent v1 candidate for:

1. `LBP1` fixed-size header and fixed-size section table;
2. little-endian integer encoding;
3. reserved v1 flags must be zero;
4. exactly one manifest and one payload section;
5. optional resources and signature sections;
6. BLAKE3-256 content identity;
7. TOML manifest with explicit required application fields;
8. v1 bundle types limited to `application` and `component`;
9. semantic `MAJOR.MINOR.PATCH` bundle versioning;
10. small semver-compatible dependency constraints;
11. opaque request-only capability strings;
12. logical mappings only, never physical DATA paths;
13. optional Ed25519 signatures with content-oriented coverage;
14. deterministic tar semantics with zstd compression for the payload section;
15. fail-closed handling of unknown versions, malformed section ranges, unsafe paths, duplicates and integrity failures.

These decisions intentionally remain candidate-level until the writer/reader and test gate are complete.

## 7. Validation rule

Unknown future bundle formats must be rejected rather than guessed. A production installer must validate the container, manifest, integrity and trust state before making an installed bundle available to runtime.

## 8. Acceptance gate

Before RFC-0002 can become Accepted, the repository should contain:

- the final normative RFC text with candidate decisions promoted to accepted requirements;
- a tested writer/reader for the outer container;
- manifest parse/validation tests;
- deterministic payload construction and traversal-safety tests;
- integrity-failure tests;
- signature coverage tests once the crypto dependency is introduced;
- an update to `docs/decisions/ARCHITECTURE-DECISION-HISTORY.md`.
