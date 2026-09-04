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

It describes:

- application identity and version;
- target `SessionId`;
- selected `RuntimeSpec`;
- executable identity and arguments;
- requested resources;
- resulting `MappingTable` context;
- the authorization requests required before materialization.

The plan is an orchestration model of `luna-app-runtime`. It is not a Bundle codec or filesystem implementation.

## 3. MappingPlan

`luna-root-mapping` owns mapping semantics and returns a deterministic mapping description. It does not authorize access and does not perform Linux namespace operations.

The application plan validates:

- runtime compatibility;
- every declared logical resource;
- executable reachability through the mapping;
- mapping consistency.

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

The launcher consumes only `AuthorizedApplicationPlan` and performs the process staging/materialization sequence.

The executable launched by the runtime must be the executable recorded in the plan and must resolve through the plan's mapping.

The child process receives the materialized logical root and selected execution context.

If process creation fails after staging begins, the temporary staging root is cleaned up before returning the error.

## 7. ApplicationInstance

`ApplicationInstance` represents one concrete launched application execution.

The runtime records at minimum:

- instance identity;
- application identity/version;
- session identity;
- runtime specification;
- lifecycle state;
- supervised process identity when one exists.

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

## 9. Required tests

The implementation covers the planning and authorization boundary with tests for:

1. inactive session rejection;
2. invalid executable rejection;
3. executable-not-mapped rejection;
4. runtime/mapping mismatch rejection;
5. foreign authorization-principal rejection;
6. fail-closed denial;
7. successful creation of typed `AuthorizedApplicationPlan`;
8. exposure of the authorized-plan launcher contract.

Linux namespace/process integration tests remain required for full mount, exec and cleanup validation.

## 10. Open design questions

The following still require separate ADR/RFC decisions before becoming policy:

- exact schema for executable declaration in the Bundle manifest;
- algorithm that translates resource declarations into authorization requests;
- semantics of `Ask` and user-confirmation IPC;
- enforcement of `Constrained` decisions;
- cgroup/resource-limit contract;
- restart policy;
- exact logical-root mount set and portal integration.
