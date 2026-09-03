# `luna-common`

**Status:** implemented foundation

## Purpose
Minimal shared value types used across Luna boundaries.

## Owns
- `BundleId`
- `ComponentId`
- `RuntimeKind`
- `RuntimeSpec`
- `UserId`
- `Version`

These are values/identities, not subsystem services.

## Must not own
Filesystem operations, authorization policy, runtime state, configuration storage, IPC transport, Bundle lifecycle, update logic or generic catch-all errors.

## Dependencies
Intentionally minimal. It is the bottom of the domain dependency graph.

## Contracts
Types crossing multiple architectural boundaries must remain implementation-neutral. Subsystem-specific validation and policy belongs to the owning subsystem.

`RuntimeKind` is a property of an application runtime specification. It does not define a new runtime component.

## Consumers
Foundation, mapping, security, state/event, Bundle, session, manager and runtime crates may depend on the shared values they actually require.

## Open
Any proposed addition must demonstrate genuine cross-component ownership before being added.
