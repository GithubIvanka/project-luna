# `UserSession`

**Status:** implemented domain/lifecycle model

## Purpose
Represent one user's interactive session as a single Luna domain entity.

## Ownership
`UserSession` is the session boundary and is owned/coordinated by `luna-system-runtime`. It is not a separate session manager process.

## State
The current implementation models session identity, user identity, session state and login state. Authentication must precede Active state.

Conceptually:

```text
Starting
  ↓
Authenticating
  ↓ success
Active
  ↓
Restricted / Ending
  ↓
Ended
```

Login failure/cancellation must not activate the session.

## Desktop relationship
GUI/Desktop session belongs to the UserSession lifecycle. The accepted graphical path is:

```text
system-runtime
 ↓
UserSession
 ↓
luna-login
 ↓
authentication
 ↓
Active UserSession
 ↓
niri-session
 ↓
niri
 ↓
Noctalia
```

## Application relationship
A UserSession may own multiple application-runtime activities. `luna-app-runtime` receives the session identity/boundary when launching ApplicationInstances.

## Multi-user behavior
Multiple UserSessions may coexist. When a user leaves the active desktop, application behavior is independently configurable: continue, remain alive but restricted, or terminate. Default is restricted.

## Does not own
Application execution implementation, system-wide process supervision, authentication policy authority, bootloader behavior or GUI toolkit implementation.

## Open
Full session switching/restriction enforcement, logout/re-authentication and production authentication IPC remain integration work.
