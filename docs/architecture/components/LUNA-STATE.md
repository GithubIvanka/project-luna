# `luna-state`

**Status:** implemented durable state boundary

## Purpose
Provide durable logical state with atomic transactions and revision-based concurrency control.

## Owns
- `StateKey`, `StateValue`;
- `Revision`;
- `StateMutation`;
- transaction semantics;
- `StateStore` and `StateError`.

The first durable backend is `redb`.

## Contract
A store exposes a monotonically increasing global revision. A transaction uses the revision observed by its caller. A mismatch returns `RevisionConflict` and applies nothing. A successful non-empty transaction advances the revision exactly once; an empty transaction does not advance it.

The store is synchronous; higher layers own async orchestration.

## Does not own
Boot state, security policy, configuration semantics, system-domain model, checkpoint implementation or update transaction orchestration.

## Storage
Persistent system state is stored under `DATA/system/state`. This database/state layer is distinct from Btrfs checkpoints and rollback snapshots.

## Dependencies
Shared value types and the selected durable backend. It must not depend upward on managers/runtimes.

## Open
Schema/version migration policy and broader reconciliation integration remain to be completed.
