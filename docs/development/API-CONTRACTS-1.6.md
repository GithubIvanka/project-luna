# Project Luna — API Contracts (Phase 1.6)

**Status:** foundation/domain baseline implemented
**Source of truth:** `docs/ARCHITECTURE.md`
**Component contracts:** `docs/architecture/components/`

This document records implementation-facing API boundaries. It must not silently redefine architecture.

## 1. Global rules

1. Public APIs expose domain contracts, not implementation accidents.
2. Ownership is singular: one subsystem owns a piece of state or policy.
3. `luna-common` contains only genuinely shared value types.
4. Filesystem primitives are not authorization policy.
5. Logical-root mapping is not authorization.
6. Configuration lookup is not permission evaluation.
7. Error types belong to their owning crate unless genuinely foundational.
8. Lower layers must not acquire higher-layer dependencies for convenience.
9. GUI and CLI are clients of backend contracts.
10. Linux mechanisms are implementation primitives; they do not redefine Luna's domain model.

## 2. Foundation contracts

### `luna-common`

Owns minimal shared values such as `BundleId`, `ComponentId`, `RuntimeKind`, `RuntimeSpec`, `UserId` and `Version`.

Must not contain filesystem operations, security policy, runtime state, configuration storage, IPC transport or subsystem-specific errors.

### `luna-fs`

Owns low-level filesystem primitives and errors. The current boundary includes `FileSystem` operations such as open/create/remove/metadata and a host-backed implementation for early development.

It does not decide authorization, logical mapping, application ownership or Bundle lifecycle.

### `luna-root-mapping`

Owns logical/physical path concepts, mapping rules and validated per-namespace mapping tables. File mappings are the default; explicit subtree mappings are allowed for semantic resource classes.

A mapping is not a security grant.

### `luna-namespace`

Owns Linux namespace/materialization primitives. Every ApplicationInstance receives an isolated filesystem/mount namespace. Other namespaces are policy-driven. It enforces supplied plans; it does not own authorization.

### `luna-config`

Owns scoped configuration and layered lookup. Common precedence is user/application override → application default → system default where applicable. It does not authorize changes.

## 3. Policy/state/event contracts

### `luna-security`

Owns principals, resources, permissions, authorization decisions, policy revisions and trust policy. Signature validity, trust and authorization remain separate.

### `luna-state`

Owns durable state and atomic revisioned transactions. The first durable backend is `redb`. It does not own boot state, security policy or update orchestration.

### `luna-event`

Owns event/subscription domain contracts. Event transport/broker is not fixed by this crate. Event classes and delivery guarantees follow the accepted event contract.

## 4. `luna-bundle`

Owns Bundle representation, manifest validation and the accepted RFC-0002 `.lbp`/LBP1 codec.

RFC-0002 is **Accepted (2026-08-30)**. It defines the transport/archive representation, deterministic payload, integrity and optional Ed25519 signature section. It does not define application lifecycle, namespace construction, Security policy or System Images.

System Images remain SquashFS and are not `.lbp` Bundles.

## 5. `luna-user-session`

Owns the UserSession domain model and lifecycle transitions. The system runtime owns/co-ordinates session instances.

Authentication must precede Active state. Multiple UserSessions may coexist. Session policy supports continue, restricted and terminate behavior when a user leaves the active desktop.

## 6. Runtime contracts

### `luna-system-runtime`

Single system-wide runtime/supervisor. It coordinates UserSessions, supervises processes and owns system runtime state.

It must not be replaced by a separate session manager.

### `luna-app-runtime`

Owns ApplicationInstance lifecycle and execution setup. It requires an active UserSession and uses mapping/security/namespace contracts before exec. `RuntimeKind` is a value property of `RuntimeSpec`, not a component.

### `luna-login`

Provides the graphical login integration for the UserSession authentication phase. Current greetd/Noctalia Greeter handoff is an integration stage; final Luna authentication IPC remains open.

## 7. Management contracts

### `luna-app-manager`

Owns Bundle installation/import, registration, update/removal/migration and application-data lifecycle controls. It does not own normal application process execution.

### `luna-system-manager`

Owns the system-domain model and queries. It does not own runtime supervision or update transaction execution.

### `luna-update-manager`

Owns state-changing update transaction execution: prepare → checkpoint → apply → verify → commit, reconciliation and rollback coordination.

### `luna-kernel-manager`

Owns kernel inventory, metadata and image/kernel compatibility queries. It does not load kernels; `luna-boot` owns UEFI boot execution.

### `luna-device-manager`

Owns device/volume discovery and lifecycle, including automatic external-volume orchestration.

## 8. User-facing/service-domain contracts

### `luna-cli`

Thin client over backend operations. It must not become a second domain manager.

### `luna-files`

GTK4 file-manager client. It consumes filesystem/volume/security-aware backend contracts. Yazi is packaged as current tooling/engine infrastructure; direct `yazi-core` integration is not claimed until implemented.

### `luna-audio`, `luna-network`, `luna-bluetooth`

These define Luna-owned domain/provider boundaries. PipeWire/WirePlumber, NetworkManager and BlueZ are implementation infrastructure, not additional Luna components.

## 9. Boot boundary

`luna-boot.efi` is a separate UEFI project outside the ordinary userspace workspace. Its contract is documented in `docs/architecture/LUNA-BOOT.md`.

## 10. Dependency direction

```text
luna-common
    ↑
foundation / policy / domain crates
    ↑
managers
    ↑
system-runtime / app-runtime
    ↑
CLI / GUI clients
```

Lower layers must not depend upward for convenience.

## 11. Implementation gate

Before implementing a boundary, its component document must answer:

- what it owns;
- what it does not own;
- public inputs/outputs;
- owning error types;
- allowed lower-level dependencies;
- deferred persistence/transport details.

See `docs/development/AI-DEVELOPMENT-RULES.md` for AI-assisted development requirements.
