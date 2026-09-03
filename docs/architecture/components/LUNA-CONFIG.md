# `luna-config`

**Status:** implemented foundation

## Purpose
Represent and resolve scoped Luna configuration without owning authorization.

## Owns
- configuration keys/values;
- configuration scopes;
- layered lookup;
- configuration-store abstraction.

## Precedence
For application settings where the class supports layering:

```text
User/Application override
        ↓
Application default
        ↓
System default
```

Machine-wide mutable configuration belongs under `DATA/system/config`; user-scoped configuration belongs under `DATA/users/<user>/config`. Exact file layout is owned by each subsystem's contract.

## Does not own
Security decisions, filesystem permissions, application lifecycle, update transactions or IPC transport.

## Dependencies
Shared value types and low-level storage primitives as required. It must not depend upward on managers/runtimes.

## Contract
Reading configuration never implies authorization. Writing configuration must be authorized by the caller through `luna-security` where applicable.

## Open
Final TOML serialization API and complete per-subsystem file layout remain implementation work.
