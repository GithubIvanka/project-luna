# `luna-app-runtime`

**Status:** implemented ApplicationInstance boundary; integration in progress

## Purpose
Own execution and lifecycle of running application instances.

## Owns
- `ApplicationInstance` identity/state;
- application process lifecycle;
- execution-environment preparation;
- association of an instance with a `UserSession`;
- runtime selection from `RuntimeSpec`.

`RuntimeKind` is a property of `RuntimeSpec`, not a new architecture component. Current kinds include Luna, Glibc and Bundle runtime semantics.

## Launch boundary

```text
ApplicationInstance
 ↓ declaration/resources
 ↓ luna-root-mapping
 ↓ luna-security
 ↓ luna-namespace
 ↓ exec
 ↓ luna-system-runtime supervision
```

A launch requires an active `UserSession` and must use the validated security/mapping context.

## Does not own
Bundle installation/removal, UserSession creation, system-wide supervision, authorization policy, raw filesystem primitives or UEFI boot.

## Dependencies
`luna-common`, `luna-user-session`, mapping, security, namespace and system-runtime contracts.

## Failure behavior
Application-runtime failure must not automatically destroy unrelated UserSessions. System-runtime may restart runtime activity according to policy.

Recovered runtime metadata does not imply restoration of in-memory application process state.

## Open
Production namespace/security integration, resource limits, restart policy and complete application lifecycle IPC remain.
