# ADR-0008 — Application PID Model and RAM System Hydration

**Status:** Accepted  
**Date:** 2026-09-06

## Decision

Luna does **not** introduce a separate `luna-app-init` component and does not add an extra application PID-1 supervisor layer.

The system-wide process supervisor is `luna-system-runtime`. It is the system's PID 1 during normal userspace operation.

The application runtime hierarchy remains:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationPlan
    ↓
AuthorizedApplicationPlan
    ↓
ApplicationLaunchContext
    ↓
luna-namespace
    ↓
ApplicationInstance
    ↓
application process
```

`luna-app-runtime` is an architectural/runtime component responsible for ApplicationInstance lifecycle and execution setup. It is not a second system supervisor and it does not require a dedicated init process between itself and the application.

The application is launched directly as the process represented by `ApplicationInstance`. Under the normal Luna isolation model it remains in the system PID namespace and therefore receives an ordinary host PID that is not 1 because `luna-system-runtime` owns PID 1.

A PID namespace is **not a mandatory part of application isolation**. Mount namespace isolation is mandatory; other Linux namespaces and resource controls are policy-driven according to the ApplicationInstance security/resource profile. If Luna later requires a PID namespace for a concrete capability, that change requires a separate architecture decision and must not silently introduce a new `luna-app-init` component.

The purpose of the application isolation boundary is not to make applications look like containers. Applications should receive a conventional Linux process/filesystem environment without exposure to Luna's physical storage layout or implementation details.

## `/` and System Image hydration

The working Linux root is a RAM-backed filesystem created by `luna-init`. A System Image is a directly stored immutable SquashFS source on SYSTEM; it is not the long-term backing filesystem for the active `/`.

The physical/logical model is:

```text
SYSTEM (immutable) ──┐
                     ├── source / hydration ──→ RAM-backed logical /
DATA (persistent) ───┘                          └→ logical /data
```

`luna-init` prepares the RAM filesystem, attaches DATA directly at its `/data`, materializes the boot-critical base, and then performs `pivot_root` so that RAM filesystem becomes the actual Linux `/`.

SYSTEM and the selected SquashFS are mounted outside the future logical root. `luna-init` keeps trusted directory FDs to those read-only sources, detaches the old initramfs tree after `pivot_root`, and passes the source FDs to `luna-system-runtime`. The physical source mounts therefore remain an implementation boundary rather than becoming a path in the user-visible filesystem tree.

There is no `/run/luna-system`, `/run/luna-image`, overlay root, or equivalent extra root layer in the architecture. `/run` is only the ordinary volatile runtime directory.

The initial RAM base contains the resources required to start `luna-system-runtime` and the boot-critical system path. Additional immutable system resources may be materialized lazily as they become required. Materialized resources become ordinary filesystem objects in the active runtime root; applications never receive the physical SYSTEM path as their root.

## SYSTEM access policy

SYSTEM is an internal OS storage boundary.

```text
ordinary user        → no access
applications         → no access
normal runtime       → controlled read-only source access
luna-updater         → sole component authorized to modify SYSTEM
```

Boot-time source mounts are read-only. The updater's ability to modify SYSTEM is a separate privileged update path and is not exposed through normal application/runtime filesystem mappings.

## Consequences

- `luna-init` owns construction of the RAM-backed logical root and the initial immutable-source handoff.
- `luna-system-runtime` is the single system-wide supervisor and normal PID 1.
- `luna-app-runtime` owns ApplicationInstance lifecycle; it is not an init process.
- There is no `luna-app-init` architectural component.
- Applications are launched directly by the application runtime through the namespace/materialization boundary.
- PID namespaces are not required by default for application isolation.
- The active logical `/` is RAM-backed rather than a mounted System Image root.
- DATA is attached directly as logical `/data` and remains persistent mutable storage.
- System Image content is materialized eagerly only for the boot-critical base and lazily thereafter where appropriate.
- Physical SYSTEM/source paths are never part of the user/application filesystem contract.
- SYSTEM modification authority belongs only to `luna-updater`.

## Implementation status

The current `develop` implementation has completed the direct RAM-root direction described by this ADR:

- `luna-init` creates a dedicated tmpfs root for the logical `/`.
- DATA is attached directly under that RAM root as logical `/data`.
- SYSTEM and the selected SquashFS image are mounted read-only outside the future logical root.
- A bounded boot-critical resource set is copied into the RAM root before handoff.
- `/proc`, `/sys`, `/dev`, `/run` and `/tmp` are created as runtime state.
- `pivot_root` makes the RAM filesystem the actual `/`.
- Trusted source FDs are retained across the root transition for the future hydration layer.
- The old initramfs tree is detached, so source mounts are not pathname-visible from the logical root.
- `luna-init` executes `/sbin/init`, which is `luna-system-runtime`.

This is intentionally not the final implementation of hydration. The current bootstrap set is explicit and conservative; it does not yet provide a complete manifest-driven dependency closure for every runtime binary or desktop component.

## Open implementation work

- define a versioned, manifest-driven boot-critical materialization contract;
- compute and validate the required dependency closure so every bootstrap executable is independently runnable from the RAM root;
- implement lazy immutable resource hydration through the trusted source boundary;
- enforce the SYSTEM updater-only write authority with a dedicated privileged update path;
- preserve file metadata and object types for the full supported materialization set and make rollback transactional;
- add privileged integration tests for RAM-root construction and application mount isolation;
- integrate image-retirement checks with update/retention state.
