# `luna-kernel-manager`

**Status:** accepted boundary; kernel build/inventory integration in progress

## Purpose
Own the Luna kernel domain: inventory, metadata and compatibility queries.

## Owns
- kernel inventory;
- kernel metadata;
- compatibility queries between kernels and System Images;
- kernel-domain validation used by update/boot paths.

## Does not own
UEFI execution (`luna-boot`), Linux kernel internals, System Image payload creation, runtime process supervision or update transaction orchestration.

## Storage
Kernels live on SYSTEM under `kernels/`. The current implementation builds upstream Linux and records `bzImage`, `System.map`, config and release metadata.

Kernel versions are independent from System Image versions.

## Dependencies
System Image metadata, `luna-state`/system manager where required, and kernel build/inventory tooling.

## Contract
Compatibility must be explicit. The existence of a kernel file does not make it valid for every System Image. Current kernel must not be removed by ordinary cleanup; Factory retains its known-good kernel.

## Open
Final persistent kernel metadata schema and complete boot/update integration remain.
