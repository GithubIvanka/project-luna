# `luna-system-runtime`

**Status:** implemented core; production integration incomplete

## Purpose
Single system-wide runtime and supervisor. It coordinates UserSessions and supervises runtime activity without absorbing manager responsibilities.

## Owns
- system runtime lifecycle;
- supervised process lifecycle;
- session registry/orchestration;
- creation/authentication/ending of UserSessions;
- association of supervised runtime activity with sessions;
- coordination of the graphical desktop session under an active UserSession.

## Hierarchy

```text
luna-system-runtime
├── UserSession A
│   ├── luna-app-runtime
│   └── GUI/Desktop session
└── UserSession B
    ├── luna-app-runtime
    └── GUI/Desktop session
```

There is no separate Luna session manager or `luna-run-session` component.

## Session contract
A graphical login session begins in an authenticating state. Authentication must succeed before the UserSession becomes Active. The runtime must reject graphical-session startup for a non-active session.

## Application boundary
`luna-app-runtime` owns ApplicationInstance lifecycle and execution setup. System-runtime provides the supervision/system boundary; it does not become the application manager.

## Privilege/session boundary
The implementation may use Linux primitives/helpers to establish the identity and environment of an active UserSession, but those helpers are implementation details and must not become additional Luna architecture components.

## Does not own
Bundle installation, authorization policy, raw filesystem mapping, UEFI boot, kernel inventory or desktop-shell implementation.

## Dependencies
`luna-user-session`, process/Linux primitives, state/event contracts and the relevant lower-level security/namespace contracts.

## Open
Production privilege transition, complete multi-session switching/restriction behavior, durable supervision/reconciliation and final graphical-login IPC integration remain.
