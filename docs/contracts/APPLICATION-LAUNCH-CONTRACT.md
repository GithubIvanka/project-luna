# Application Launch Contract

**Status:** Draft for implementation
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
luna-namespace
    ↓
process spawn / exec
    ↓
ApplicationInstance
```

No later stage may be used to grant authority that was not present in an earlier stage.

## 2. ApplicationPlan

`ApplicationPlan` is the execution decision produced from a validated bundle declaration and an active `UserSession`.

It describes:

- application identity and version;
- target `SessionId`;
- selected `RuntimeSpec`;
- executable identity;
- requested resources;
- resulting `MappingPlan`;
- the authorization context required to materialize the execution environment.

`ApplicationPlan` is an orchestration model owned by the application execution boundary. It must not become a duplicate Bundle codec or filesystem implementation.

## 3. MappingPlan

`MappingPlan` is produced from the application resource declarations plus the execution context. It is a deterministic description of the logical filesystem view.

`luna-root-mapping` owns mapping semantics. It does not authorize access and does not perform Linux namespace operations.

The plan must be completely validated before security evaluation. Invalid, ambiguous, or contradictory mappings fail before authorization.

## 4. Security boundary

`luna-security` evaluates the complete authorization request set derived from the execution plan.

```text
request ≠ grant
```

A security decision must be available before Linux namespace materialization. `Deny`, policy errors, and unsupported constrained decisions fail closed.

A successful policy decision does not itself create mounts or processes.

## 5. Namespace materialization

`luna-namespace` receives only an already-authorized mapping context. Its responsibility is enforcing the mapping through Linux namespace primitives.

Physical paths in `DATA` remain implementation details. Applications operate through their logical root.

Namespace creation/materialization must not consult an alternate policy path or silently expand the authorized mapping.

## 6. Process launch

The executable launched by the runtime must correspond to the executable declared by the application execution plan. A caller must not be able to bypass the declaration by supplying an unrelated absolute path.

The child process receives the materialized logical root and the execution context selected by the plan.

Unexpected process, namespace, filesystem, or policy failures return typed errors and must trigger cleanup of resources already acquired for the failed launch.

## 7. ApplicationInstance

`ApplicationInstance` represents one concrete launched application execution.

The runtime owns its lifecycle and records at minimum:

- instance identity;
- application identity/version;
- session identity;
- runtime specification;
- lifecycle state;
- supervised process identity when one exists.

The instance is not considered fully running until the process has been successfully created and associated with the instance.

## 8. Ownership model

The hierarchy remains:

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

Borrowed references (`&T`, `&mut T`) should be used for transient access to plans, manifests, policy and runtime services. Long-lived application state belongs to the runtime owner.

## 9. Required tests

The implementation must test at least:

1. invalid bundle declaration is rejected before namespace materialization;
2. invalid/ambiguous mapping is rejected before security;
3. denied security request causes no namespace materialization;
4. policy error fails closed;
5. executable not declared by the application is rejected;
6. successful authorization permits exactly the supplied mapping;
7. process-launch failure cleans up staging resources;
8. successful launch creates one `ApplicationInstance` with the correct session/runtime identity;
9. process exit transitions the instance and cleans its execution root.

## 10. Open design questions

The following require separate ADR/RFC decisions before they become policy:

- exact schema for executable declaration in the Bundle manifest;
- algorithm that translates resource declarations into authorization requests;
- semantics of `Ask` and user-confirmation IPC;
- enforcement of `Constrained` decisions;
- cgroup/resource-limit contract;
- restart policy;
- exact logical-root mount set and portal integration.
