# ADR-0008 — Application PID Boundary and RAM System Hydration

**Status:** Accepted
**Date:** 2026-09-05
**Supersedes:** the earlier wording that PID namespace use is optional for application presentation

## Decision

Luna separates the process that acts as namespace supervisor from the application process itself.

For an ApplicationInstance that uses a PID namespace:

```text
PID namespace
│
├── PID 1  → Luna application namespace supervisor/init
│            owns child reaping and namespace lifetime
│
└── PID 2+ → actual application process(es)
```

The application process must **never intentionally run as PID 1** inside its application PID namespace. PID 1 is reserved for the namespace supervisor. This prevents the application from being given the special Linux PID-1 role and gives Luna an explicit place for child reaping, signal handling and lifecycle supervision.

The application nevertheless receives a conventional Linux process environment. PID numbering is an implementation detail of the namespace and must not be exposed as a special "container mode" marker by Luna itself.

## `/` and System Image hydration

The working Linux root is RAM-backed. System Image SquashFS is an immutable source, not the long-term backing filesystem for `/`.

Boot/runtime construction follows this model:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
          │
          │ selected immutable source
          ▼
  early userspace / system materializer
          │
          ├── copy/materialize boot-critical system base into RAM
          ├── create RAM-backed compatibility directories
          └── lazily materialize additional immutable system resources
                when they become part of the active runtime
          │
          ▼
       RAM-backed logical /
          │
          └── luna-system-runtime
```

The initial RAM base includes the resources required to boot and operate the system runtime. Pseudo-filesystems and volatile directories such as `/dev`, `/proc`, `/sys`, `/run` and `/tmp` are created/mounted in RAM rather than being persistent copies of SYSTEM.

Lazy materialization is permitted for immutable resources that are not needed by the boot-critical base. It must produce normal filesystem objects in the runtime root rather than expose the physical SYSTEM path to applications.

A resource that has already been materialized into the active runtime remains valid independently of the lifetime of the original System Image file. Removing an active System Image is allowed only after the system/update layer has established that no still-required runtime resource remains dependent on it.

This means the running system is not conceptually a mounted SquashFS root. The System Image is a source for materialization and hydration of immutable system content.

## `/dev`

`/dev` is a runtime-generated Linux device namespace. It is not a direct view of the host `/dev` by default. Device nodes and device access are provided only according to the device/security policy.

## Consequences

- application PID 1 is reserved for Luna's namespace supervisor;
- ApplicationInstance lifecycle has a dedicated PID-namespace supervision boundary;
- `/` remains RAM-backed;
- boot-critical system content is materialized before normal system services depend on it;
- remaining immutable system resources may be hydrated lazily;
- an application never receives the physical SYSTEM filesystem as its root;
- a System Image may be retired after runtime materialization proves that its active content is no longer required.

## Open implementation work

- implement the PID namespace supervisor/child-spawn path in `luna-system-runtime`/`luna-namespace`;
- define the exact system-materialization manifest for the boot-critical RAM base;
- implement lazy immutable resource hydration without exposing SYSTEM paths;
- add privileged integration tests for PID namespace, `/proc` visibility, child reaping and RAM-root hydration;
- integrate image-retirement checks with update/retention state.
