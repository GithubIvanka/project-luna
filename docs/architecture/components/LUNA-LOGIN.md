# `luna-login`

**Status:** implementation/integration boundary exists; final auth IPC is open

## Purpose
Provide the graphical login integration used by the UserSession authentication phase.

## Owns
- starting the configured graphical greeter integration;
- presenting/consuming the authentication handoff defined by the current integration contract;
- reporting authentication success/failure back to `luna-system-runtime`.

## Does not own
UserSession creation, system-wide session supervision, authorization policy, application lifecycle or desktop session management.

## Current integration
The current implementation integrates greetd/Noctalia Greeter through a controlled handoff under `/run/luna-login`. The runtime creates the UserSession in an authenticating state, invokes `luna-login`, and only then transitions the UserSession to Active.

This is an integration stage, not permission to invent a permanent shell-wrapper login architecture.

## Desktop handoff
After successful authentication the active UserSession starts the graphical session (`niri-session` → niri → Noctalia) under the authenticated user identity.

## Security
Authentication identity must be checked against the authenticated user/session context. A successful greeter process exit alone is not sufficient evidence of authentication.

## Open
Final production authentication IPC and integration with the shared Luna security/IPC model remain to be specified and implemented.
