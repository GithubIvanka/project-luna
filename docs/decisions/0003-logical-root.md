# ADR-0003 — Logical Linux Root

**Status:** Accepted  
**Phase:** 1.1  
**Date:** 2026-08-18

## Decision

Luna provides a conventional Linux-compatible logical `/` without physically recreating the Linux directory hierarchy in DATA.

The initial root is RAM-based. System Image content is supplied through hybrid/lazy access so the whole SquashFS image does not need to be eagerly copied into RAM.

The exact kernel/SquashFS/mount implementation remains open.

## `luna-root`

`luna-root` owns logical-root construction and controlled path mapping. It does not own application lifecycle, sessions, updater logic or recovery logic.

## Compatibility paths

Paths such as `/etc`, `/home`, `/usr`, `/lib`, `/bin` and `/var` are logical compatibility interfaces. Their physical backing is selected by policy rather than by mirroring DATA into `/`.
