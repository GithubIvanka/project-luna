# RFC-0002 — Bundle Format v1 Candidate Decision Set

**Status:** Candidate / review required. This document does **not** mark RFC-0002 as Accepted.

The purpose of this companion is to turn the open RFC-0002 questions into one coherent implementation candidate without silently changing the architectural Source of Truth. The final acceptance should update `docs/rfc/RFC-0002.md` and the architecture/decision history together.

## 1. Candidate container

The primary v1 container remains the structured `.lbp` layout already proposed by RFC-0002:

```text
Header
Section table
MANIFEST
PAYLOAD
RESOURCES (optional)
SIGNATURE (optional)
```

The container is intentionally not a generic filesystem image. An installed Bundle remains an immutable installed object; `.lbp` is only its transport/archive representation.

### 1.1 Header candidate

All integer fields are little-endian.

```text
magic             4 bytes    ASCII "LBP1"
version           u16        1
flags             u16        reserved, must be zero in v1
section_count     u32
section_table_off u64
header_hash       32 bytes
```

`header_hash` is the digest of the header with the `header_hash` field zeroed. This removes the self-reference ambiguity present in a naive header hash.

### 1.2 Section table candidate

Each entry is fixed-size:

```text
section_type        u32
compression         u32
offset              u64
compressed_length   u64
uncompressed_length u64
content_hash        32 bytes
```

Section types:

```text
1 MANIFEST
2 PAYLOAD
3 RESOURCES
4 SIGNATURE
```

Compression values:

```text
0 none
1 zstd
```

v1 requires exactly one MANIFEST and one PAYLOAD section. RESOURCES and SIGNATURE are optional. Unknown section types are rejected by a v1 implementation rather than ignored.

Offsets and lengths must remain inside the `.lbp` file, sections must not overlap, and section ranges must not wrap integer arithmetic.

## 2. Integrity candidate

**Candidate decision: BLAKE3-256.**

Reasoning:

- native 32-byte digest fits the proposed table without translation;
- fast hashing is useful for large application payloads;
- strong Rust ecosystem support;
- the format can still expose the digest as an opaque 32-byte content identity rather than leaking implementation details into the rest of Luna.

The digest is used for:

- `content_hash` in every section;
- the Bundle content identity;
- signature coverage.

The implementation must not truncate the digest.

## 3. Manifest candidate

The manifest remains **TOML**. The candidate schema is intentionally small and explicit:

```toml
format = 1

[bundle]
id = "org.example.app"
name = "Example App"
version = "1.2.3"
type = "application"

[platform]
arch = "x86_64"
min_system = "1.0.0"

[entry]
exec = "bin/example"
logical = "/usr/bin/example"

[[dependency]]
id = "org.luna.libs.foo"
version = ">=1.0.0"

[capabilities]
requested = ["network"]

[[mapping]]
logical = "/usr/bin/example"
source = "bin/example"
access = ["execute"]

[metadata]
author = "Example"
license = "Apache-2.0"
```

Required fields for an `application` bundle:

- `format`;
- `bundle.id`;
- `bundle.name`;
- `bundle.version`;
- `bundle.type`;
- `platform.arch`;
- `entry.exec`.

`entry.logical`, dependencies, capabilities, mappings and metadata are optional.

The parser must reject duplicate tables/keys where the TOML representation would be ambiguous to the Bundle domain model.

## 4. Bundle type vocabulary

**Candidate decision:** v1 defines only:

```text
application
component
```

Other types such as `library`, `runtime`, `driver`, `theme` and `font` remain future format/schema extensions instead of being guessed into v1.

## 5. Versioning

**Candidate decision:** Bundle version uses the existing Luna `Version` semantic value (`MAJOR.MINOR.PATCH`).

The format does not allow arbitrary version ordering rules in v1. Version constraints in dependency declarations use a deliberately small semver-compatible constraint grammar owned by the Bundle parser; unsupported constraint syntax is rejected.

## 6. Dependencies

Dependency entries identify another Bundle by stable Bundle/Application identity plus a version constraint.

A dependency declaration is not itself an authorization grant and does not force network access. Resolution/install policy belongs to `luna-app-manager` and security policy remains owned by `luna-security`.

## 7. Capabilities

**Candidate decision:** the manifest vocabulary remains request-only. v1 does not invent a second permission model.

The manifest carries opaque capability request strings. `luna-security` is the authority that maps a request to a policy decision. Unknown capability names are not automatically granted.

## 8. Logical mappings

Bundle mappings remain logical-interface declarations:

```text
logical path -> bundle-relative source
```

A manifest must never encode a physical `DATA/system/apps/...` path.

The application/runtime layer converts these declarations into validated `luna-root-mapping` rules. Authorization stays outside the bundle format.

## 9. Content identity

**Candidate decision:** content identity is the full 32-byte BLAKE3 digest of the canonical payload representation defined by the format implementation.

Bundle identity is conceptually:

```text
(ApplicationID, Version, ContentIdentity)
```

The precise binary derivation may be exposed as a typed `BundleContentIdentity` value rather than forcing every subsystem to reproduce the hash concatenation rules.

ApplicationInstanceID and UserID remain runtime identities and are not part of the bundle identity.

## 10. Signature candidate

The signature remains optional in v1.

**Candidate algorithm:** Ed25519.

The SIGNATURE section is excluded from its own signed message. Signature coverage is:

```text
header with header_hash normalized
+ section table with SIGNATURE entry content bytes excluded from coverage
+ MANIFEST bytes
+ PAYLOAD bytes
+ RESOURCES bytes when present
```

The signature authenticates content; it does not grant permission and does not by itself establish local trust.

Publisher identity, signature validity, repository metadata and local trust remain separate concepts.

## 11. Payload representation

**Candidate decision:** PAYLOAD is a deterministic compressed file-tree representation. The initial implementation should use `tar` semantics with zstd compression, while the outer `.lbp` container remains responsible for section addressing and hashing.

Requirements:

- bundle-relative member paths only;
- no absolute paths;
- no `..` traversal;
- deterministic member ordering;
- deterministic metadata policy;
- no symbolic-link escape outside the payload root;
- duplicate member names rejected.

RESOURCES is reserved for future independently-addressed resource material and is not required for a minimal v1 application.

## 12. Parser behavior

A v1 parser/install path must fail closed:

```text
read fixed header
    ↓
validate magic/version/flags
    ↓
validate section table and ranges
    ↓
verify header hash
    ↓
read + parse manifest
    ↓
verify section hashes
    ↓
validate payload tree
    ↓
(optional) verify signature
    ↓
produce Bundle domain object
```

Unknown format versions, malformed TOML, invalid paths, invalid section ranges, overlapping sections, duplicate members and failed integrity checks are hard failures.

No malformed bundle may become an installed runtime object.

## 13. Storage/import

`.lbp` is a transport artifact. After successful installation, the installed immutable bundle is registered through `luna-app-manager` and the original transport file may be discarded.

Removable-media bundles may be launched/registered according to the lifecycle and security policy. Importing a bundle never bypasses integrity/trust checks.

## 14. Compatibility

v1 accepts only `format = 1` and `LBP1`.

A future format version is a different parser contract. v1 must reject it rather than silently guessing.

## 15. Acceptance gate

Before changing RFC-0002 from Draft to Accepted, the repository should contain:

1. the final RFC text with the chosen decisions promoted from Proposal to Accepted;
2. a tested writer/reader for the outer container;
3. manifest parse/validation tests;
4. payload traversal and determinism tests;
5. integrity-failure tests;
6. signature coverage tests once the crypto dependency is introduced;
7. an update to `docs/decisions/ARCHITECTURE-DECISION-HISTORY.md`.

Until that gate is met, this file is a candidate implementation specification, not a new architectural invariant.
