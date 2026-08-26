# ADR-0006 — User Sessions and Btrfs Checkpoints

**Status:** Accepted  
**Phase:** 1.2  
**Date:** 2026-08-18

## User sessions

Multiple users may be active simultaneously.

Each user independently configures what happens to their applications when they stop being the active desktop session:

1. continue normally;
2. remain alive but restricted;
3. terminate.

Default: option 2.

A configuration change should normally affect only the relevant session/services where possible. A full reboot is reserved for changes that genuinely require it. System services such as an updater may continue across user switches when safe.

## Btrfs checkpoints

Btrfs snapshots are a checkpoint/rollback subsystem for selected mutable DATA state.

They are:

- not runtime session-switching machinery;
- not a replacement for immutable System Image rollback;
- not a conventional long-term backup system.

The user can choose the previously discussed targeted option 2, broader option 3, or disable checkpoints. Default: option 2.

Exact checkpoint scope, naming, retention and automatic-creation policy remain open.
