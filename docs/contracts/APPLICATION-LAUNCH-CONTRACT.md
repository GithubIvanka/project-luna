# Application Launch Contract

**Status:** Partially implemented in `luna-app-runtime`
**Scope:** `UserSession` → `ApplicationInstance`

## Launch pipeline

```text
Bundle declaration
    ↓
ApplicationPlan
    ↓ validation
luna-root-mapping / MappingPlan
    ↓ authorization by luna-security
AuthorizedApplicationPlan
    ↓ internal trusted setup
private mount namespace + RAM-backed logical /
    ↓ process creation
application process in the normal system PID namespace
    ↓ execve()
ApplicationInstance
```

`ApplicationPlan` is request/orchestration state, not authorization. The launcher accepts only the opaque `AuthorizedApplicationPlan` produced after `luna-security` evaluates the complete request set.

## ApplicationPlan and mapping

`ApplicationPlan` contains application identity/version, `UserSession` identity, `RuntimeSpec` (including `RuntimeKind`), executable and arguments, declared resources, mapping context, and authorization requests.

Mapping validation precedes authorization. `luna-root-mapping` owns deterministic logical-to-physical mapping semantics; a `MappingPlan` is not a grant. The current implementation receives a prepared mapping table. Automatic executable → ELF interpreter → shared-library/runtime-resource dependency closure is not implemented yet.

## Authorization

`luna-security` evaluates runtime, mapping/resource, capability, and explicit requests. `Deny`, policy errors, `Ask`, and unsupported `Constrained` outcomes fail closed. Authorization performs no namespace or process operation.

Capability provider registration is lookup only. It does not create a `CapabilityGrant` or make a policy decision.

## Session boundary

Launch revalidates that the supplied `UserSession` is active and has the identity captured by the authorized plan. Inactive and foreign sessions fail before staging or process creation. A missing session is excluded by the typed API because launch requires `&UserSession`.

## Trusted setup and process launch

Trusted setup is an internal `luna-app-runtime` stage, not a daemon, service, global helper, `luna-runtime`, or `luna-app-init`. It receives only `AuthorizedApplicationPlan` plus the system-selected `ApplicationLaunchContext`.

`ApplicationLaunchContext` contains the mount namespace configuration, immutable System Image source root, per-launch staging parent, and explicit trusted physical source roots. All roots must be absolute and navigation-free. Host `/`, staging content as a source, and staging inside the System Image root are rejected.

The production launcher creates a fresh staging directory, enters a private mount namespace, materializes a fresh tmpfs logical `/`, attaches only runtime-profile resources and authorized mappings, enters the logical root, applies filesystem enforcement, and then executes the application through the supervised process API.

The default process model does **not** create a PID namespace. The application is an ordinary child in the normal system PID namespace. There is no per-application PID 1 and no PID namespace supervisor. Additional PID/user/network/IPC/UTS/time namespaces remain policy-driven future work; mount namespace isolation is mandatory.

Source and target attachment use FD-based resolution with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)`, `open_tree`, `mount_setattr`, and `move_mount`. Mount propagation is made private before materialization.

The built-in minimal runtime profile exposes the trusted runtime subtrees `/usr`, `/lib`, and `/lib64` read/execute. It does not expose the whole system `/etc`; required configuration files must be explicit authorized mappings.

An uncommitted staging root is removed on setup/spawn/attachment/transition failure. The runtime removes committed roots after process exit or requested termination. Cleanup errors are currently not surfaced and remain open work.

## ApplicationInstance

`ApplicationInstance` records instance identity, application identity/version, session identity, `RuntimeSpec`, lifecycle state, typed process identity after creation, terminal exit/crash outcome, and typed launch/supervision failure information.

Supported core transitions are:

```text
Created → Starting → Running → Stopping → Stopped
Starting → Failed
Running → Crashed
Stopping → Failed
```

A naturally exiting successful process may move `Running → Stopped`; a non-zero or signaled exit moves `Running → Crashed`. Terminal states cannot return to `Running`, and callers cannot mutate state directly.

## Validation coverage

Unit/contract tests cover valid and invalid lifecycle transitions, active/inactive/foreign session handling, authorization denial before launch, authorized-plan-only launcher typing, runtime/mapping mismatch, executable mapping, mapping access checks, launch-context trust roots, normal and abnormal exit recording, and uncommitted staging cleanup.

Privileged integration tests for real namespace/mount/chroot/Landlock behavior are not part of ordinary CI yet.

## Open work

- executable → ELF interpreter → library/runtime-resource dependency closure;
- system-runtime ownership/allocation of system-wide `ApplicationInstanceId` values;
- cgroup v2 placement and resource limits, including possible `clone3`/`CLONE_INTO_CGROUP` integration;
- final credential, capability, device, `/proc`, `/sys`, `/run`, and inherited-FD policy;
- a setup/exec error channel that distinguishes pre-exec setup failure from final `execve()` failure;
- durable lifecycle recovery after runtime restart;
- observable cleanup failures and leaked-mount recovery;
- removal or strict test-only restriction of the legacy whole-System-Image OverlayFS helper path;
- privileged integration tests for real mount namespace, root transition, mount rollback, and Landlock enforcement.
