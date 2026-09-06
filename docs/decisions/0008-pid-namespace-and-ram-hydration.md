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

The working Linux root is RAM-backed. A System Image is a directly stored immutable SquashFS source on SYSTEM; it is not the long-term backing filesystem for the active `/`.

Boot/runtime construction follows this model:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
          │
          │ selected immutable source
          ▼
       luna-init
          │
          ├── mount System Image at an internal source location
          ├── create RAM-backed logical /
          ├── create runtime pseudo-filesystems
          ├── materialize boot-critical system content into RAM
          └── hydrate additional immutable resources lazily
          │
          ▼
       RAM-backed logical /
          │
          └── luna-system-runtime (PID 1)
```

`luna-init` is therefore the bootstrap boundary between the Linux kernel and the normal Luna runtime. It does not switch the machine to a persistent disk-backed Linux root.

The initial RAM base contains the resources required to start `luna-system-runtime` and the boot-critical system path. Additional immutable system resources may be materialized lazily as they become required. Materialized resources become ordinary filesystem objects in the active runtime root; applications never receive the physical SYSTEM path as their root.

Pseudo-filesystems and volatile paths such as `/dev`, `/proc`, `/sys`, `/run` and `/tmp` are generated/mounted as runtime state rather than copied from SYSTEM.

A resource already materialized into the active runtime remains valid independently of the lifetime of the source System Image. An image may be retired only after the system/update layer has established that no still-required runtime content depends on it.

## Consequences

- `luna-init` owns system bootstrap and construction of the RAM-backed logical root.
- `luna-system-runtime` is the single system-wide supervisor and normal PID 1.
- `luna-app-runtime` owns ApplicationInstance lifecycle; it is not an init process.
- There is no `luna-app-init` architectural component.
- Applications are launched directly by the application runtime through the namespace/materialization boundary.
- PID namespaces are not required by default for application isolation.
- The active logical `/` is RAM-backed rather than a mounted System Image root.
- System Image content is materialized eagerly only for the boot-critical base and lazily thereafter where appropriate.
- System Image paths remain internal implementation details.

## Open implementation work

- replace the current early-userspace `switch_root` prototype with the RAM-root materialization path;
- define the exact boot-critical materialization manifest and dependency closure;
- implement lazy immutable resource hydration without exposing SYSTEM paths;
- add privileged integration tests for RAM-root construction and application mount isolation;
- integrate image-retirement checks with update/retention state.
