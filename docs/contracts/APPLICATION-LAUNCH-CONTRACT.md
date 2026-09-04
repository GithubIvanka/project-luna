# Application Launch Contract

**Status:** Draft implemented in `luna-app-runtime`
**Scope:** `UserSession` → `ApplicationInstance`

## 1. Purpose

This contract defines the execution boundary for launching a Luna application. The launch path must remain explicit and ordered:

```text
Bundle declaration
    ↓
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
    ↓
luna-namespace
    ↓
process spawn / exec
    ↓
ApplicationInstance
```

No later stage may be used to grant authority that was not present in an earlier stage.

## 2. ApplicationPlan

`ApplicationPlan` is the explicit execution decision produced from a validated Bundle declaration and an active `UserSession`.

It describes application identity/version, session identity, `RuntimeSpec`, executable path and arguments, resource declarations, mapping context, and explicit authorization requests.

The plan is an orchestration model of `luna-app-runtime`. It is not a Bundle codec or filesystem implementation.

## 3. MappingPlan

`luna-root-mapping` owns mapping semantics and returns a deterministic mapping description. It does not authorize access and does not perform Linux namespace operations.

The application plan validates runtime compatibility, every declared logical resource, executable reachability through the mapping, and mapping consistency.

Invalid, ambiguous, or contradictory mappings fail before security evaluation.

## 4. Security boundary

`luna-security` evaluates the complete authorization request set derived from the execution plan.

```text
request ≠ grant
```

A successful policy decision returns an `AuthorizedApplicationPlan`. `Deny`, policy errors, and unsupported constrained decisions fail closed.

The authorized plan is a distinct type so the process launcher cannot accidentally accept an unapproved plan.

## 5. Namespace materialization

`luna-namespace` receives only an already-authorized execution context. Its responsibility is enforcing the logical filesystem view through Linux namespace primitives.

Physical paths in `DATA` remain implementation details. Applications operate through their logical root.

## 6. Process launch

The plan launcher consumes only `AuthorizedApplicationPlan` and performs staging, logical-root materialization in the child, and supervised process creation.

The executable launched by the runtime must be the executable recorded in the plan and must resolve through the plan's mapping.

If process creation fails after staging begins, the temporary staging root is cleaned up before returning the error.

## 7. ApplicationInstance

`ApplicationInstance` represents one concrete launched application execution.

The runtime records instance identity, application identity/version, session identity, runtime specification, lifecycle state, and supervised process identity when one exists.

`Running` is reached only after the supervised process has been created and attached.

## 8. Ownership model

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`luna-system-runtime` remains the system-wide supervisor. `luna-app-runtime` owns application execution lifecycle. There is no generic `luna-runtime` daemon.

## 9. Implemented tests

The planning boundary covers inactive-session rejection, executable-path validation, executable-not-mapped rejection, runtime/mapping mismatch rejection, foreign-principal rejection, fail-closed denial, successful creation of typed `AuthorizedApplicationPlan`, and compile-time exposure of the authorized-plan launcher contract.

Linux namespace/process integration tests remain required for privileged mount/exec and full cleanup validation.

## 10. Open design questions

The following still require separate ADR/RFC decisions before becoming policy:

- exact schema for executable declaration in the Bundle manifest;
- algorithm that translates resource declarations into authorization requests;
- semantics of `Ask` and user-confirmation IPC;
- enforcement of `Constrained` decisions;
- cgroup/resource-limit contract;
- restart policy;
- exact logical-root mount set and portal integration.
