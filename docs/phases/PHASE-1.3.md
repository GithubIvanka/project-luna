# Project Luna — Phase 1.3

## Status

**Accepted and consolidated into `docs/ARCHITECTURE.md`.**

Phase 1.3 defined the component/service architecture, management boundaries, runtime split, security layer, state model, resource protection, CLI model, and the rule that managers and runtimes must have narrow responsibilities.

## Accepted component model

```text
luna-cli
├── luna-system-manager
├── luna-app-manager
├── luna-device-manager
├── luna-update-manager
├── luna-kernel-manager
├── luna-root-mapping
├── luna-security
├── luna-system-runtime
├── luna-app-runtime
├── luna-fs
├── luna-bundle
├── luna-config
├── luna-log
└── luna-common
```

Manager components may use a small daemon + library model. CLI and future GUI are thin clients over the same backend APIs.

## Major decisions

- `luna-root-mapping`: narrow logical-root and mapping responsibility; not a general manager.
- Mapping is file-oriented, per namespace, policy-controlled and kept primarily in RAM.
- `luna-app-manager`: installation, update, removal, verification, compatibility and migrations; it does not own execution.
- `luna-app-runtime`: application execution environment, namespace, mappings, permissions and lifecycle.
- `luna-system-runtime`: one system-level runtime supervising multiple users' app runtimes.
- `luna-system-manager`: owner of system state/model/query semantics.
- `luna-update-manager`: executor of system/kernel/application update transactions where appropriate.
- `luna-kernel-manager`: kernel state/model/compatibility; update-manager executes kernel installation/removal/update transactions.
- `luna-device-manager`: device detection, filesystem identification, mount, volume exposure, permissions and safe removal.
- `luna-security`: central policy authority; kernel/filesystem primitives enforce restrictions.
- `luna-fs`: low-level filesystem abstraction crate.
- `luna-bundle`: internal bundle representation/format concerns; lifecycle belongs to app-manager.
- `luna-config`: configuration subsystem; early placeholder APIs may be redesigned to match current architecture.
- `luna-common`: deliberately small; not a dumping ground.

## Runtime model

One system runtime supervises multiple user app runtimes. `ApplicationInstance` represents an individual running instance. Async/multithreaded/multicore execution is an explicit design goal.

Closing an application normally releases its resources. Runtime mapping/permission state may remain in RAM for configurable/adaptive retention, with memory-pressure eviction.

## Permissions

Permissions are a separate layer. Visibility, readability and writability are distinct. Application-wide rules can be shared across instances where appropriate.

## Users and authority

There is no permanent root user and no required `sudo`/`su` architecture. User roles/permissions define authority. Administrative operations may use a system/admin credential without introducing a root account.

## Resource protection

The system reserves CPU/memory/GPU resources for itself. Existing Linux mechanisms are preferred initially. ZRAM/swap policy remains configurable.

## State and events

Persistent state is used where it is the source of truth. Changes are event-driven rather than rewritten on every invocation. Event streams may use Kafka-like concepts where useful without requiring Kafka itself.

## CLI

The executable is `luna-cli`. Human-friendly aliases such as `app i`, `app u`, `app d`, `sys -u`, and `dev list` are supported conceptually, and aliases are user-configurable with descriptions.
