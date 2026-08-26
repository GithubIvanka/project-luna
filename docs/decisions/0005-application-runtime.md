# ADR-0005 — Application Runtime and Lifecycle

**Status:** Accepted  
**Phase:** 1.2  
**Date:** 2026-08-18

## Decision

The Application Manager owns:

- installation;
- launch;
- update;
- removal;
- application-data lifecycle;
- orphaned-data discovery and cleanup controls.

The runtime/namespace layer owns application process and namespace lifecycle. `luna-root` remains focused on logical root and mapping composition.

## Immutable bundles

Application bundles are immutable lifecycle units regardless of their physical location. Bundles may be moved to other disks or removable media without moving mutable user state.

Mutable state is stored separately:

```text
DATA/users/<user>/data/<app>/
DATA/users/<user>/config/<app>/
```

The default logical application home is `/home/<user>/`.

## Missing dependencies

Luna must not silently download arbitrary missing dependencies. It should determine the requirement, locate a suitable source where possible, explain it to the user and ask before downloading/installing.

## Application access

Application access to external volumes, devices and user locations is permission-controlled. The conceptual states are visible, readable and writable.
