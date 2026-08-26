# ADR-0002 — Boot, Recovery and Factory State

**Status:** Accepted  
**Phase:** 1.2  
**Date:** 2026-08-18

## Decision

Normal initialization is:

```text
luna-boot.efi
    ↓
minimal RAM logical root
    ↓
System Image
    ↓
attach DATA
    ↓
normal users
```

If DATA cannot be used, Luna enters Recovery using the System Image without normal persistent DATA and with a temporary virtual recovery user whose state lives in RAM.

If the System Image itself cannot start, Luna uses the immutable Factory pair:

```text
Factory System Image
Factory Kernel
```

Factory is the original known-good installation state and is never removed, replaced or modified by ordinary lifecycle management.

## Recovery capabilities

Recovery is a functional Luna environment, not merely a diagnostic shell. It should support diagnosis, repair, removal of broken DATA components such as incompatible drivers, user-data recovery and external media access.

## Failure philosophy

Failures should be diagnosed and repaired where possible before presenting an unrecoverable emergency state. Even in an emergency state, Luna should preserve user agency where technically possible.
