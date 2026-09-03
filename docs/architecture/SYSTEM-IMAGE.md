# Project Luna — System Image

**Status:** accepted core format direction; detailed specification still required
**Canonical payload:** `luna-X.Y.Z.squashfs`

## 1. Definition

A Luna System Image is the immutable filesystem payload of one Luna system version.

It is **directly a SquashFS filesystem image**.

It is never:

- an `.lbp` Bundle;
- a Bundle containing SquashFS;
- a generic container whose payload happens to be SquashFS.

## 2. On-disk representation

Each normal image is stored beside its manifest:

```text
SYSTEM/images/
├── luna-X.Y.Z.squashfs
└── luna-X.Y.Z.toml
```

The image filename is versioned. The adjacent TOML manifest carries metadata needed by boot/update logic.

## 3. Image contents

The SquashFS tree contains the immutable Luna userspace/system payload needed to construct the logical Linux root.

It may include, according to the system build contract:

- system binaries and libraries;
- configuration defaults;
- runtime components;
- desktop/login assets;
- immutable resources.

Mutable machine/user/application state must remain outside the image.

## 4. Manifest contract

The manifest is the per-image metadata authority for:

- Luna version;
- image identity;
- supported/required architecture;
- kernel compatibility information;
- boot-related metadata;
- integrity information where defined;
- retention/role metadata where defined.

The exact final TOML schema is still a separate specification task. Implementations must not invent mandatory fields merely because a boot implementation would like them.

## 5. Kernel independence

System Image version and kernel version are independent.

```text
System Image A ── compatible ── Kernel 1
System Image A ── compatible ── Kernel 2
System Image B ── compatible ── Kernel 2
```

Compatibility is explicit. The loader must never assume that the newest kernel is valid for every image.

## 6. Loading model

The architecture allows hybrid/lazy access to SquashFS content rather than eagerly copying the entire image into RAM.

The logical-root layer may materialize required content as needed. If active system content has been materialized into RAM and its source image is later removed, that materialized content must not be reclaimed when no other valid source exists.

## 7. Factory image

Factory is a preserved original known-good System Image paired with its factory kernel.

```text
Factory System Image
+
Factory Kernel
```

Factory is never replaced or removed by ordinary update/retention operations.

## 8. Retention

System Images are versioned and retained according to policy. The current usable state and fallback state must remain available until the update/health contract confirms that they can safely be removed.

Exact retention counts are policy, not an implicit filesystem rule.

## 9. Update boundary

`luna-update-manager` executes state-changing update transactions. `luna-system-manager` owns system state/query semantics. `luna-kernel-manager` owns kernel inventory and compatibility queries.

The System Image format itself does not own update transactions.

## 10. Verification

Before an image is made bootable, the boot/update path must establish that the image is structurally valid and that its metadata is internally consistent. Integrity/authenticity policy must be explicit; the image format must not be conflated with application `.lbp` trust policy.

## 11. Relationship to `.lbp`

```text
Application / component Bundle → .lbp → installed Bundle
Luna System Image              → .squashfs + .toml → SYSTEM image
```

These are independent formats and independent lifecycle domains.
