# `luna-app-manager`

**Status:** accepted boundary; implementation/integration in progress

## Purpose
Manage application Bundle lifecycle and mutable application-data lifecycle.

## Owns
- install/import;
- verification and registration;
- update/removal;
- migration;
- application-data discovery/cleanup policy;
- ingestion of supported Linux packages such as `.deb`/`.rpm` into Luna Bundle form.

## Does not own
Normal process execution, UserSessions, namespace enforcement, authorization policy, low-level filesystem primitives or system update transaction orchestration.

## Install boundary
The safe Bundle flow is:

```text
inspect → validate → integrity/trust checks → security decision → stage → atomic commit
```

A failed install must not leave a partially registered Bundle.

## Storage
Installed immutable Bundles live under `DATA/system/apps`. User data/config remain under the corresponding user DATA tree.

## Dependencies
`luna-bundle`, `luna-fs`, `luna-config`, `luna-security`, `luna-state` and update contracts as required. It must not become the app runtime.

## Open
Complete dependency resolution, transaction/reconciliation behavior and package-import hardening remain.
