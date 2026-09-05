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
ApplicationLaunchContext
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

The launcher accepts only `AuthorizedApplicationPlan` plus an `ApplicationLaunchContext` selected by the system runtime.

`ApplicationLaunchContext` groups the process-local Linux mount namespace, the immutable System Image base root, per-launch staging parent, and explicit trusted physical source roots. Bundle-supplied physical paths never become trust roots implicitly.

The context requires absolute, navigation-free roots. Staging must remain outside the System Image base-root tree, host `/`, and the staging tree itself. Trusted source roots must be explicitly selected by the system runtime and must not be host `/` or staging content.

The launcher stages an empty execution root, materializes the logical `/` as RAM-backed tmpfs inside the child namespace, mounts only the profile and authorized mappings, and creates the supervised process through `luna-system-runtime`.

Source attachment resolves below the explicit source trust root. Target attachment independently resolves below the per-launch logical-root destination. Both sides use FD-based resolution with `openat2(... RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)`, `open_tree`, `mount_setattr` where needed, and `move_mount`. No production target is resolved against host `/`.

Materialization is transactional. Every successful mount is registered in a local rollback transaction; any later materialization error unmounts already-created mounts in reverse order before returning failure. The higher-level application runtime owns cleanup of the staging directory when the supervised process exits or is terminated.

The executable must be absolute, traversal-free, and present in the authorized mapping. Spawn failure cleans the temporary staging root.

## ApplicationInstance

`ApplicationInstance` records instance identity, application identity/version, session identity, runtime specification, lifecycle state, and supervised process identity.

The plan launcher marks the instance `Running` only after successful process creation and process attachment.

## Tests

Planning and authorization coverage includes inactive sessions, executable validation, executable mapping, runtime mismatch, principal binding, fail-closed denial, successful authorization, authorization ordering, fail-closed pipeline termination, launch-context root isolation, explicit trusted source-root validation, and the authorized launcher API surface.

Privileged Linux namespace/process tests remain a separate integration stage.

## Open decisions

- Bundle executable declaration schema;
- resource declaration → authorization request translation;
- `Ask` and confirmation IPC;
- `Constrained` enforcement;
- cgroup/resource-limit contract;
- restart policy;
- final logical-root mount/portal set;
- privileged integration tests for real unshare/mount/chroot/Landlock behavior;
- complete filtered `/dev` policy.
