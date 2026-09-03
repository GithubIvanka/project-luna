# `luna-namespace`

**Status:** implemented initial Linux backend; integration hardening required

## Purpose
Materialize the controlled execution environment described by mapping/security contracts using Linux primitives.

## Owns
- mount/filesystem namespace creation;
- controlled bind/mount composition;
- OverlayFS/idmapped-mount use where accepted and useful;
- controlled `/dev` exposure;
- namespace-specific materialization mechanics.

## Does not own
- authorization policy;
- Bundle install/update;
- UserSession lifecycle;
- ApplicationInstance lifecycle;
- logical mapping semantics.

## Required isolation
Every ApplicationInstance receives an isolated filesystem/mount namespace. Other Linux namespaces (user, PID, network, IPC/UTS/time) are policy-driven rather than blindly enabled for every process.

`cgroups v2` is the accepted resource-control primitive but its policy is not owned by this crate.

## Contract
This layer enforces the plan it is given. It must not turn an arbitrary DATA path into a visible logical path, and it must not grant capabilities simply because a process is being launched.

No application receives `CAP_SYS_ADMIN` or equivalent host-level privilege by default.

## Dependencies
Consumes `luna-root-mapping`, `luna-fs`, security-derived decisions and Linux kernel primitives. It must not become the security authority.

## Open
Complete namespace profiles, user/PID/network isolation policy and production mount materialization are still being hardened.
