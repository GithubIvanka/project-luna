# `luna-security`

**Status:** implemented policy foundation; enforcement integration incomplete

## Purpose
Central Luna authorization, permission and trust-policy authority.

## Owns
- principals and resources;
- permission dimensions: Visibility, Read, Write, Execute, Device Use, Manage;
- authorization requests and decisions;
- policy revisions/snapshots;
- trust decisions as distinct from cryptographic signature validity.

## Does not own
Filesystem mapping, namespace creation, raw I/O, GUI presentation, application execution or Bundle parsing.

## Contract
Bundle mappings/capabilities/access declarations are requests, never grants. `Ask` requires explicit confirmation. `Constrained` contains structured restrictions. A per-instance policy may tighten an application policy but cannot weaken an enforced deny.

Trust binds content (including Bundle identity/content identity and trust scope). Signature validity, trust and authorization are separate decisions.

## Enforcement chain

```text
application declaration
 ↓
luna-root-mapping
 ↓
luna-security
 ↓
luna-namespace / runtime enforcement
```

Low-level kernel/filesystem mechanisms enforce the resulting decision.

## Dependencies
Consumes shared identities and mapping/resource descriptions. Must remain independent of GUI/CLI and must not become a process supervisor.

## Open
Durable policy storage, user confirmation IPC/UI, trust store and complete kernel enforcement are integration work.
