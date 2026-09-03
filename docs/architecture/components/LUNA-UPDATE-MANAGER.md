# `luna-update-manager`

**Status:** accepted boundary; implementation/integration in progress

## Purpose
Execute state-changing update transactions across system, application and kernel domains.

## Owns
- update operation execution;
- prepare/checkpoint/apply/verify/commit sequencing;
- interrupted-operation reconciliation;
- rollback coordination;
- retention/removal decisions after health/commit confirmation.

## Transaction model

```text
prepare
  ↓
checkpoint
  ↓
apply
  ↓
verify
  ↓
commit
```

The previous authoritative state remains available until commit is confirmed where possible.

## Does not own
The system-domain model (`luna-system-manager`), kernel metadata queries (`luna-kernel-manager`), application process execution, security policy, or raw bootloader logic.

## Dependencies
Consumes `luna-state`, system/app/kernel managers, Bundle/Image metadata, security and checkpoint facilities.

## Rollback
Rollback is explicit/user-visible. Automatic rollback may be policy-triggered by boot/health contracts. Btrfs snapshots are the accepted checkpoint direction where applicable, but checkpoints are not the same thing as the state database or System Image rollback.

## Open
Complete multi-domain transaction journal/reconciliation and end-to-end image/kernel update integration remain.
