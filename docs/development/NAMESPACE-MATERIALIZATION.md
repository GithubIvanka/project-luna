# Project Luna — Linux Namespace / Materialization Prototype

**Status:** prototype implementation
**Source of Truth:** `docs/ARCHITECTURE.md`

## Purpose

This document records the first real Linux backend for the accepted logical-root model. It is intentionally an implementation document, not a replacement for the architecture.

## Boundary

```text
luna-root-mapping
    ↓
validated logical → physical mapping
    ↓
luna-namespace
    ↓
Linux mount namespace + bind mounts
```

`luna-root-mapping` owns path/mapping semantics. `luna-namespace` owns Linux-specific namespace and mount syscalls. `luna-security` remains the authorization authority and must be consulted before a mapping is accepted for materialization.

## Current prototype

`luna-namespace` currently provides:

- entry into a new Linux mount namespace using `unshare(CLONE_NEWNS)`;
- conversion of the copied mount tree to private propagation with `MS_PRIVATE | MS_REC`;
- application of validated mapping rules as bind mounts;
- read-only remounting of those bind mounts.

The mount namespace is expected to be created in a dedicated child/process context before application execution. The API intentionally does not attempt to restore the caller's original namespace.

## Why mount namespaces

Linux mount namespaces provide each namespace with an isolated view of the mount list, while bind mounts can expose selected existing resources at controlled locations. This directly matches the first Luna implementation strategy without requiring a custom virtual filesystem. citeturn415334search2turn415334search6

## Why private propagation

A new mount namespace initially inherits the parent mount tree. The prototype makes the tree private before applying mappings so changes made for the application do not propagate back to the parent namespace. Linux documents this as the standard protection against mount propagation across namespaces. citeturn415334search2turn415334search12

## ID-mapped mounts

ID-mapped mounts remain an optional future implementation mechanism. Linux associates an ID mapping with a mount and can expose the same underlying files with different ownership views without changing ownership globally. citeturn415334search1turn415334search4

They are not required by the current prototype.

## Important limitations

This prototype is **not yet the complete Luna application root materializer**. It does not yet:

- construct the complete logical Linux `/` tree;
- mount `/proc`, `/sys`, `/dev`, or `/run` with Luna policy;
- create user namespaces;
- create PID/network/IPC/UTS namespaces;
- enforce `luna-security` decisions itself;
- implement OverlayFS composition;
- create per-user writable data mappings;
- handle mount cleanup/lifecycle around `execve()`;
- provide privileged helper separation;
- implement a hardened syscall/FD lifecycle;
- guarantee that every destination path exists before binding;
- resolve symlinks against an authorized physical root.

Those are subsequent runtime/backend tasks.

## Security boundary

The current low-level API assumes that the mapping table has already been authorized. It intentionally does not make policy decisions from the paths it receives.

In particular, a successful bind mount is not proof that Luna security policy allows the resource. Authorization must happen before materialization, and the runtime must never construct a broader mapping than the authorized mapping plan.

## Development requirements

The backend is Linux-specific and requires the process to have the capabilities/privileges necessary for the requested namespace and mount operations. The exact privilege-drop and helper architecture remains a future runtime design task.

## Next implementation steps

1. introduce a higher-level materializer that builds the full Linux-compatible logical root before bind operations;
2. pass an already-authorized immutable MappingPlan instead of a mutable MappingTable directly;
3. isolate privileged namespace setup from the post-materialization application process;
4. add per-application writable data/cache mappings;
5. add policy-driven `/dev`, `/proc`, `/sys`, Wayland and PipeWire views;
6. evaluate ID-mapped mounts and OverlayFS only where they materially simplify the design;
7. integrate with `luna-app-runtime` process creation and lifecycle.
