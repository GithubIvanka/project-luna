# ADR-0004 — Per-Application Namespace and File Mapping

**Status:** Accepted  
**Phase:** 1.1 / 1.2  
**Date:** 2026-08-18

## Decision

Every application receives its own filesystem/mount namespace.

Each namespace has a small mapping table containing only the files/resources required by that application. There is no global mapping table.

Mappings are file-oriented rather than blind whole-directory mappings.

Conceptual resolution order:

```text
application
    ↓
user
    ↓
system
```

Example:

```text
App A: /lib/gtk → DATA/system/libs/gtk/3
App B: /lib/gtk → DATA/system/libs/gtk/4
```

## Policy

Mapping policy determines what physical resource may satisfy what logical path. Permission policy determines which namespace may see, read or write it.

An arbitrary DATA path must not automatically become an arbitrary Linux path.

Applications must not see unrelated application files or other users' protected data merely because those files exist physically.

## Namespace state

Mapping tables are runtime state and normally live in RAM. They may be retained after application exit and evicted adaptively under memory pressure. Retention is configurable; the previously discussed one-hour value is not a fixed rule.
