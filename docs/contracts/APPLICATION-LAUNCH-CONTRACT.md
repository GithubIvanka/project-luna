# Application Launch Contract

**Status:** Draft implemented in `luna-app-runtime`
**Scope:** `UserSession` → `ApplicationInstance`

## Launch pipeline

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

The application execution plan is validated before authorization. The authorized plan is a distinct type consumed by the process launcher.

## ApplicationPlan

`ApplicationPlan` contains application identity/version, session identity, `RuntimeSpec`, executable path and arguments, resource declarations, mapping context, and explicit authorization requests.

The plan is orchestration state owned by the application runtime boundary. It is not a Bundle codec, namespace implementation, or security policy store.

## Mapping

The plan verifies runtime compatibility, every declared logical resource, executable reachability through the `MappingTable`, and mapping consistency.

Mapping validation precedes security evaluation. `luna-root-mapping` remains responsible only for deterministic logical-to-physical mapping semantics.

## Authorization

`luna-security` evaluates the complete request set. `Deny`, policy errors, and unsupported `Constrained` decisions fail closed.

Successful authorization creates `AuthorizedApplicationPlan`. No namespace or process operation is performed as part of policy evaluation.

## Process launch

The launcher accepts only `AuthorizedApplicationPlan`. It stages an execution root, materializes the logical root in the child, and creates the supervised process through `luna-system-runtime`.

The executable must be absolute, traversal-free, and present in the authorized mapping. Spawn failure cleans the temporary staging root.

## ApplicationInstance

`ApplicationInstance` records instance identity, application identity/version, session identity, runtime specification, lifecycle state, and supervised process identity.

The plan launcher marks the instance `Running` only after successful process creation and process attachment.

## Tests

Planning and authorization coverage includes inactive sessions, executable validation, executable mapping, runtime mismatch, principal binding, fail-closed denial, successful authorization, and the authorized launcher API surface.

Privileged Linux namespace/process tests remain a separate integration stage.

## Open decisions

- Bundle executable declaration schema;
- resource declaration → authorization request translation;
- `Ask` and confirmation IPC;
- `Constrained` enforcement;
- cgroup/resource-limit contract;
- restart policy;
- final logical-root mount/portal set.
