# Project Luna — Typed Runtime Contract

**Дата:** 2026-09-01  
**Статус:** принято / реализовано  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

## 1. Purpose

The runtime contract describes the execution environment selected for an `ApplicationInstance`. It does not add another architectural subsystem to Luna.

The typed value is:

```text
RuntimeKind
├── Luna
├── Glibc
└── Bundle
```

`RuntimeSpec` currently contains the selected `RuntimeKind`.

## 2. Runtime hierarchy vs execution pipeline

These are deliberately different concepts.

### Ownership hierarchy

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

This is the architectural ownership hierarchy. `luna-system-runtime` remains the single system-wide runtime/supervisor; `UserSession` is the combined user/session entity; `luna-app-runtime` owns ApplicationInstance lifecycle and execution setup.

### Application execution pipeline

```text
ApplicationInstance
    ↓
Bundle/resource declarations
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
luna-namespace
    ↓
process execution
    ↓
luna-system-runtime supervision
```

This pipeline is not a second hierarchy. The system runtime remains the owner of process supervision even when the application runtime prepares an individual execution environment.

## 3. RuntimeKind semantics

`RuntimeKind` is a small typed value carried by the application execution contract. It is not a manager, daemon, service or independent process boundary.

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved glibc compatibility environment
RuntimeKind::Bundle → Bundle-private runtime where permitted
```

The selected runtime is an attribute of the ApplicationInstance execution environment.

One process uses one libc/runtime environment. Different ApplicationInstances may use different runtime kinds concurrently.

## 4. Ownership

The ownership boundaries remain:

```text
luna-common
    ↓ shared RuntimeKind / RuntimeSpec value types only
luna-app-runtime
    ↓ ApplicationInstance lifecycle and execution setup
luna-root-mapping
    ↓ logical mapping semantics
luna-security
    ↓ authorization policy
luna-namespace
    ↓ Linux namespace/materialization
luna-system-runtime
    ↓ process supervision and UserSession orchestration
```

There is **no separate generic `luna-runtime` crate or runtime daemon**.

## 5. Mapping and security

A runtime selection does not grant access by itself.

The normal order is:

```text
runtime attribute
    ↓
mapping validation
    ↓
Security authorization
    ↓
namespace materialization
    ↓
execution
```

`luna-security` remains the central policy authority. Runtime use can be represented as `Resource::Runtime(RuntimeKind)` with `Permission::Use` when the selected runtime is a protected resource.

Mapping declarations remain requests, not grants.

## 6. ApplicationInstance

`ApplicationInstance` stores its selected `RuntimeSpec` for its lifecycle. The system runtime owns global `ApplicationInstanceId` uniqueness; the application runtime owns the individual instance lifecycle after creation.

The runtime selection therefore follows the existing hierarchy:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance { RuntimeSpec }
```

## 7. Manifest boundary

The final RFC-0002 TOML field for runtime selection is not defined by this decision. A future Bundle/RFC decision must map validated manifest data onto `RuntimeSpec` without introducing a second free-form runtime subsystem.

## 8. libc rules

- `Luna` uses musl for the native system userspace.
- `Glibc` is an optional managed compatibility environment for applications that require glibc ABI/runtime behavior.
- `Bundle` may supply a private runtime where policy allows.
- libc environments must not be mixed arbitrarily inside one process.
- Runtime isolation is implemented using the existing mapping/security/namespace architecture, not a new generic runtime manager.

## 9. Implementation boundary

The current Linux application launcher may still use its development namespace child-setup mechanism. That is an implementation-hardening issue, not a reason to introduce another architectural component.
