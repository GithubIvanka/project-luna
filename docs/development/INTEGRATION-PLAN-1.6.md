# Project Luna — Integration Plan (Phase 1.6)

**Status:** Active
**Source of truth:** `docs/ARCHITECTURE.md`

This document defines the cross-crate verification sequence after the first foundation and manager/runtime API passes.

## 1. Mapping + security

Verify that a namespace can represent an exact logical file mapping while the security authority independently decides whether the caller may use the resulting resource.

Expected separation:

```text
logical path
    ↓
luna-root-mapping
    ↓
physical resource
    ↓
luna-security
    ↓
allow / deny
```

The mapping layer must never become the permission authority.

## 2. Configuration precedence

Verify:

```text
user/application override
        ↓
application default
        ↓
system default
```

Removing a user override must expose the lower layer again. System-wide settings must remain independent from the active user session.

## 3. Bundle + mapping

Verify that an immutable bundle resource can be represented as a logical path such as `/bin/app` without exposing its physical `DATA/system/apps/...` location to the application.

Duplicate logical resource entries must be rejected by bundle validation.

## 4. Session + application instance

Verify that one `UserSession` can own multiple application instances and that instance lifecycle remains independent from application installation/update policy.

## 5. System + kernel state

Verify that:

- System Manager models current and factory System Image state;
- Kernel Manager models current, previous and factory kernel state;
- update-manager remains the mutation executor;
- factory state is never treated as an ordinary disposable retention entry.

## 6. Application manager + update manager

Verify that App Manager creates/plans lifecycle work without directly performing update mutations.

```text
application request
        ↓
luna-app-manager
        ↓
application plan
        ↓
update execution path
        ↓
luna-update-manager
```

## 7. Runtime + security

Verify that application runtime requests policy decisions rather than embedding its own authorization rules.

## 8. Runtime + event layer

Verify that system runtime can consume event contracts without coupling the domain model to a particular broker implementation.

## 9. Future high-risk prototypes

After these contract tests stabilize, prototype:

1. logical-root materialization;
2. Linux namespace integration;
3. application resource accounting and system resource reservation;
4. persistent state backend;
5. update transaction/checkpoint behaviour;
6. System Image compatibility resolution;
7. boot-state persistence;
8. Bundle Format v1 implementation after RFC-0002 acceptance.
