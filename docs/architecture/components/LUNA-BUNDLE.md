# `luna-bundle`

**Status:** implemented domain + RFC-0002/LBP1 codec

## Purpose
Represent, validate, read and write Luna Bundles and the accepted `.lbp` transport format.

## Owns
- Bundle identity and metadata;
- manifest model/validation;
- Bundle resource representation;
- LBP1 container reader/writer;
- deterministic payload encoding;
- BLAKE3 content identity;
- optional Ed25519 signature codec/verification boundary;
- format hardening and path validation.

## Format invariant
`.lbp` is transport/archive representation. It is not a System Image and not the installed runtime representation.

System Images remain `luna-X.Y.Z.squashfs` plus manifest.

## Manifest rules
Mappings are logical Bundle-relative declarations. A manifest must never encode physical `DATA/system/apps/...` or `DATA/users/...` paths as mapping targets.

Capabilities/access fields are requests, not grants. Authorization is owned by `luna-security`.

## Does not own
Installation/update/removal policy, trust policy, namespace creation or process lifecycle.

## Dependencies
Shared IDs/version types and format/serialization primitives. It may expose validated data to `luna-app-manager` and runtime/mapping layers without depending upward on them.

## Integration
`luna-app-manager` owns installation transactions. Runtime consumes installed Bundle semantics. External Bundles must be inspected/integrity-checked before install/launch.

## Open
Broader repository/supply-chain trust and delta update mechanisms are outside RFC-0002.
