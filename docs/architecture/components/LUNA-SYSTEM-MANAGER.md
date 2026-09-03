# `luna-system-manager`

**Status:** accepted boundary; implementation/integration in progress

## Purpose
Own the Luna system-domain model and queries describing installed/selected system state.

## Owns
- system state model;
- system-level queries;
- domain information used by update/boot/runtime clients.

It does not itself become the runtime supervisor.

## Does not own
Kernel process execution, UserSession lifecycle, application lifecycle, authorization, raw filesystem I/O or update transaction execution.

## Dependencies
Consumes `luna-state`, System Image metadata contracts and other domain queries as required.

## Contract
System state is durable logical state, distinct from runtime state, boot state and checkpoints. State mutations that are part of an update transaction are executed through `luna-update-manager`.

## Open
Final persistent schema, query API and complete integration with System Image registration remain to be hardened.
