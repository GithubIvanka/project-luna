# ADR-0011 — Canonical PID 1 and direct userspace boot chain

**Status:** Accepted / supersedes conflicting historical wording  
**Date:** 2026-09-08

## Decision

The canonical normal-boot process hierarchy is:

```text
Linux kernel
    ↓
luna-init (PID 1)
    ↓
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`luna-init` is the first userspace process and remains PID 1 for the normal lifetime of the system.

`luna-system-runtime` is **not** PID 1. It is a child of `luna-init` and owns the higher-level system runtime/supervision domain.

The direct boot transition is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
direct initial-userspace execution
  ↓
luna-init (PID 1)
```

There is no initramfs userspace hop, no `pivot_root`, no `switch_root` and no later replacement of PID 1 with `/sbin/init`.

## Rationale

PID 1 is the trusted bootstrap boundary that knows the physical machine, validates Luna boot context and constructs the initial System Environment. Higher runtime code should not need to rediscover physical boot storage or inherit early-boot responsibilities.

Keeping `luna-system-runtime` as a child preserves a clean separation:

```text
luna-init
    machine / boot / PID-1 boundary

luna-system-runtime
    system environment / services / sessions
```

## Superseded wording

Any older document that describes `luna-system-runtime` as PID 1, or describes `luna-init` as a temporary initramfs bootstrap followed by a second `/sbin/init`, is historical and is superseded by this decision and the current `LUNA-INIT-CONTRACT.md`.

The known stale sections in the consolidated architecture document must be updated when that document is next rewritten as a complete file, rather than preserving the contradiction.

## Required implementation invariant

A successful normal boot must make the following observable relationship true:

```text
/proc/1/comm == luna-init
```

and `luna-system-runtime` must have a PID greater than 1 and a parent PID of 1.

The bring-up test suite must assert these invariants instead of accepting a userspace process named `luna-system-runtime` as PID 1.
