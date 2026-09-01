# Project Luna — Runtime Boundaries

**Date:** 2026-09-01  
**Status:** accepted correction  
**Source of Truth:** `docs/ARCHITECTURE.md`

## Decision

Project Luna does **not** introduce a generic `luna-runtime` crate or runtime resolver service.

The existing architecture already provides the required runtime boundaries:

- `luna-system-runtime` — system-wide process supervision and system runtime lifecycle;
- `luna-app-runtime` — application runtime/lifecycle and process launch;
- `luna-root-mapping` — logical-to-physical mapping policy;
- `luna-namespace` — Linux namespace materialization;
- `luna-security` — authorization policy;
- `luna-common` — small shared value types such as `RuntimeKind` where cross-component typing is required.

`RuntimeKind` remains a typed value describing the requested execution environment (`Luna`, `Glibc`, or `Bundle`). It is not evidence that a fourth runtime subsystem exists.

The resolution of the requested runtime must be performed by the component that owns the corresponding operation. For application launch, that owner is `luna-app-runtime`, using mapping/security/namespace contracts as appropriate.

## Removed direction

The previously introduced development-only `components/luna-runtime` crate is rejected and must not appear in the workspace or architecture documentation.

This correction restores the Phase 1.6 implementation boundary, which explicitly states that no generic `luna-runtime` crate is introduced.

## Consequence

Future runtime work must extend the existing `luna-app-runtime` / `luna-root-mapping` / `luna-security` path instead of creating a new generic runtime layer.
