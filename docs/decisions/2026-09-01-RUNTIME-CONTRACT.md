# Project Luna — Typed Runtime Contract

**Дата:** 2026-09-01  
**Статус:** принято / реализовано в development integration branch  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

## 1. Цель

Решение из `docs/decisions/2026-09-01-LIBC-AND-INIT.md` переводится из концептуальной модели в typed Rust contract.

A runtime environment is represented by:

```text
RuntimeKind
  ├── Luna
  ├── Glibc
  └── Bundle
```

and by `RuntimeSpec`, which currently contains the selected `RuntimeKind` only.

## 2. Ownership

`luna-common` contains only the shared runtime value types because they are consumed across subsystem boundaries. It does **not** own runtime storage, installation, lifecycle, policy or Linux namespace materialization.

Ownership remains:

```text
luna-common
    ↓ typed RuntimeKind / RuntimeSpec
luna-root-mapping
    ↓ runtime-bound logical mapping plan
luna-security
    ↓ authorization of Runtime(kind) + Use
luna-app-runtime
    ↓ execution selection / ApplicationInstance
luna-namespace
    ↓ Linux materialization
luna-system-runtime
    ↓ process supervision
```

## 3. Mapping contract

A `MappingTable` may be runtime-neutral while it is being assembled. Once a runtime is bound, the mapping accepts only that exact runtime kind.

Changing:

```text
Glibc → Luna
```

or any other runtime transition on an accepted mapping plan is rejected.

The mapping layer still does not know physical glibc/musl storage layout.

## 4. Security contract

Runtime use is represented as a normal Security resource:

```text
Resource::Runtime(RuntimeKind)
Permission::Use
```

A runtime declaration therefore does not grant itself access. The selected runtime must be authorized by `luna-security` before namespace materialization/execution.

The application principal is used for the runtime request.

## 5. Application runtime contract

`ApplicationInstance` stores its selected `RuntimeSpec` for its entire lifecycle.

The runtime-aware launch path validates:

1. Bundle validity;
2. mapping/runtime agreement;
3. runtime authorization;
4. other requested resource authorizations;
5. namespace materialization.

This preserves the required ordering:

```text
runtime selection
    ↓
mapping validation
    ↓
security authorization
    ↓
namespace materialization
    ↓
process execution
```

## 6. Compatibility

Existing callers that do not explicitly select a runtime continue to use:

```text
RuntimeKind::Luna
```

as the default. This is transitional API compatibility, not permission bypass. Security-aware execution still requires the normal runtime authorization request.

## 7. Manifest boundary

The accepted runtime taxonomy is now typed in Rust, but the final RFC-0002 TOML manifest field for runtime selection is intentionally **not** introduced by this decision. That schema requires a dedicated Bundle/RFC compatibility decision.

The future manifest layer will map its validated value onto `RuntimeSpec` rather than introduce a second free-form runtime string throughout the runtime subsystem.

## 8. libc rules

This contract does not change the accepted libc architecture:

- `RuntimeKind::Luna` → native Luna userspace, musl;
- `RuntimeKind::Glibc` → approved glibc compatibility runtime;
- `RuntimeKind::Bundle` → Bundle-private runtime where permitted.

One process uses one libc environment. Different processes may use different runtime kinds concurrently.

## 9. Current implementation boundary

The current Linux launcher still uses the existing development `pre_exec` namespace hook. This is unchanged by this contract and remains a production-hardening item.

The contract therefore establishes the semantic runtime boundary first; concrete runtime filesystem layout and production-safe child creation remain subsequent implementation work.
