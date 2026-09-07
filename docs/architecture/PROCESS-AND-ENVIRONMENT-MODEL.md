# Project Luna — Process and Environment Model

**Status:** architecture direction under active design  
**Branch:** `develop`  
**Date:** 2026-09-07

This document records the Linux process model being used to reason about `luna-init`, `luna-system-runtime`, and application runtimes.

## 1. Process is the execution unit

A Linux environment is not created by a special "VM process". A normal Linux process is created and then its execution context is shaped with kernel primitives.

Conceptually:

```text
create process
    ↓
shape execution context
    ├── namespaces
    ├── cgroup
    ├── credentials
    ├── capabilities
    ├── filesystem mappings
    └── security policy
    ↓
exec program
```

## 2. `clone` / `clone3` vs `execve`

`clone()`/`clone3()` create a new task and can request namespace isolation and other process properties. `execve()` does **not** create a new process: it replaces the program image of the current process while retaining its PID.

This distinction is central to Luna.

```text
luna-init
   │
   ├── create child execution context
   │
   └── child → execve(luna-system-runtime)
```

The child remains one process throughout this transformation; `execve()` changes what code it runs.

## 3. PID model

Current direction:

```text
initial Linux PID namespace

PID 1   luna-init
PID N   luna-system-runtime
PID M   application
```

A PID namespace is not required merely to isolate an application or to force the application to have PID 1. A new PID namespace has its own PID hierarchy and its first process becomes PID 1, so adding it changes semantics rather than being a free isolation switch.

## 4. cgroup v2

cgroup v2 organizes processes hierarchically and controls resource distribution through controllers.

Luna should prefer assigning a workload to its logical cgroup at startup rather than repeatedly moving it around solely to apply restrictions.

Conceptually:

```text
Luna root cgroup
├── system-runtime
│   ├── system services
│   └── sessions
└── applications
    ├── app A
    └── app B
```

The exact tree and delegation model remain to be specified.

## 5. Resource protection model

Resource governance is not only about preventing an application from consuming an unlimited amount of memory. Luna also needs to protect the minimum resource budget required for the system itself to remain responsive and operational.

For memory, cgroup v2 provides distinct mechanisms with different semantics:

- `memory.min` provides hard memory protection within its effective boundary;
- `memory.low` provides best-effort protection against reclaim;
- `memory.high` applies reclaim pressure and throttling without directly invoking the cgroup OOM killer;
- `memory.max` is a hard usage limit and may invoke the cgroup OOM killer when the limit cannot be satisfied otherwise.

Therefore the conceptual Luna policy is not simply `memory.max` everywhere. A future resource policy may combine:

```text
system environment
    ├── protected minimum memory
    └── enough unreserved capacity for normal operation

application environments
    ├── controlled memory.high
    ├── hard memory.max where appropriate
    └── observed pressure / usage for adaptive management
```

The exact numerical values must be hardware/profile dependent and are not architectural constants yet.

The same principle applies to CPU and I/O: Luna should preserve a usable system budget while allowing applications to consume otherwise idle capacity. cgroup v2 provides both weight-based distribution and absolute bandwidth controls such as `cpu.weight` and `cpu.max`.

## 6. `clone3()` as the preferred process-creation primitive

The current architectural direction is to prefer modern `clone3()` semantics in Luna's low-level execution layer where they provide a concrete advantage over `fork()`/`clone()`.

In particular, Linux supports `CLONE_INTO_CGROUP`, which allows a child to begin directly in a specified cgroup v2 instead of first entering the parent's cgroup and then being migrated. This matches Luna's goal of assigning a workload to its resource domain at creation time.

This is a **direction**, not yet a Rust syscall implementation contract. The exact flags and ordering of namespace, credential, cgroup, filesystem and security operations remain subject to experimentation.

## 7. Credentials and capabilities

UID/GID identify the process from the Linux credential model; capabilities split privileged operations into separate units. A process being UID 0 in one user namespace does not imply unrestricted power over the initial user namespace.

Luna therefore must reason separately about:

```text
identity
authorization
capability set
namespace membership
resource limits
```

`execve()` can recalculate capabilities, so the state before and after execution must be treated as a lifecycle boundary.

## 8. Environment construction

The target Luna model is:

```text
EnvironmentPlan
      ↓
Authorization
      ↓
resource / namespace materialization
      ↓
create execution context
      ↓
execve(target)
```

The security decision happens before the final environment is materialized, matching the established application rule.

## 9. `luna-system-runtime`

`luna-system-runtime` is a managed Linux userspace environment, not a KVM/QEMU guest.

It may use a controlled set of namespaces, cgroup v2 resource controls, credentials/capabilities, filesystem mappings and other Linux security primitives. Its view of storage should be logical rather than tied to physical SYSTEM/DATA device paths.

## 10. Canonical runtime hierarchy

The accepted ownership hierarchy is:

```text
Linux kernel
    ↓
luna-init (PID 1)
    ↓
System Environment
    ↓
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

Applications are not launched directly by `luna-init`.

`luna-system-runtime` owns system-wide supervision and `UserSession` orchestration. `luna-app-runtime` owns application execution/lifecycle orchestration. Each `ApplicationInstance` represents one concrete running application.

There is no `luna-app-init` layer and no generic `luna-runtime` layer.

## 11. Applications

Applications reuse the same conceptual execution pipeline, with stricter policy as appropriate:

```text
Bundle
  ↓
ApplicationPlan
  ↓
Authorization
  ↓
create execution context
  ↓
execve(application)
```

The concrete application environment is materialized for the `ApplicationInstance` under `luna-app-runtime` rather than being a direct child environment managed by `luna-init`.

## 12. PID 1 responsibilities

If `luna-init` remains PID 1 for the entire system lifetime, it must behave as the kernel's PID-1 userspace supervisor: in particular it must correctly handle child reaping and system signal/lifecycle semantics.

If a future design chooses to transfer PID-1 execution to another program via `execve()`, that is a separate architectural decision and must not be introduced implicitly.

## 13. Open questions

- permanent PID-1 role of `luna-init` vs bootstrap handoff;
- exact cgroup v2 hierarchy and protected system resource budget;
- exact namespace set for `luna-system-runtime`;
- whether `luna-system-runtime` needs a user namespace;
- exact capability bounding/ambient/inheritable policy;
- signal and child-reaping ownership;
- exact security mechanism composition (capabilities, seccomp, LSM, namespaces);
- exact filesystem materialization order;
- hardware/profile-specific resource policy values;
- exact `clone3()` flag set and syscall ordering.
