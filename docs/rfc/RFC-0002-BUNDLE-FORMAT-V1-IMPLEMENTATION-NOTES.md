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

The proposed outer container, section table, compression, hashing, signature block and exact manifest schema remain proposals in RFC-0002 until the RFC is explicitly accepted.

## 3. Mapping boundary

A validated bundle resource is converted by the application/runtime layer into a `LogicalPath` and resolved through `luna-root-mapping`.

The bundle layer does not own physical `DATA` paths, authorization, mounts, or namespace creation.

## 4. Security boundary

Capability declarations in a future bundle manifest remain requests only. Authorization is evaluated by `luna-security` through `PolicyAuthority` and is not inferred from the presence of a bundle resource or mapping.

A deterministic `StaticPolicyAuthority` now exists for contract tests and early prototypes. It is not the final persisted policy backend.

## 5. Runtime boundary

`luna-app-runtime` now has an in-memory lifecycle prototype that validates bundle structure and mappings, requires an active `UserSession`, creates an `ApplicationInstance`, and supports stop/failure transitions. It does not install bundles, perform mounts, or execute processes.

## 6. Open RFC decisions

Before RFC-0002 can become Accepted, the project still needs explicit decisions for at least:

1. final outer `.lbp` container layout;
2. section table encoding and reserved flags;
3. compression algorithm/version;
4. hash algorithm (RFC currently lists BLAKE3/SHA-256 as candidates);
5. exact manifest schema and required fields;
6. bundle type vocabulary;
7. versioning rules;
8. dependency constraint syntax;
9. capability vocabulary and relationship to `luna-security`;
10. signature algorithm and trust metadata;
11. signature coverage/canonicalization rules;
12. compatibility and forward-version behavior;
13. exact BundleID/content binding derivation;
14. storage/import rules for removable bundles;
15. migration rules between future format versions.

No implementation in this pass should be interpreted as silently resolving these decisions.

## 7. Validation rule

Unknown future bundle formats must be rejected rather than guessed. A production installer must validate the container, manifest, integrity and trust state before making an installed bundle available to runtime.
