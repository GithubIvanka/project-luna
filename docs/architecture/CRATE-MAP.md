# Project Luna — Current Crate Map

**Status:** current implementation map  
**Authority:** `docs/ARCHITECTURE.md`  
**Component contracts:** `docs/architecture/components/`

This document describes repository package boundaries. It does not independently redefine architecture.

## Foundation

| Crate | Responsibility | Form |
|---|---|---|
| `luna-common` | Small shared value types and identifiers | lib |
| `luna-fs` | Low-level filesystem primitives and metadata | lib |
| `luna-root-mapping` | Logical path and mapping semantics | lib |
| `luna-namespace` | Linux namespace/materialization primitives | lib |
| `luna-config` | Configuration model and scoped configuration | lib |

## Policy and state

| Crate | Responsibility | Form |
|---|---|---|
| `luna-security` | Policy, authorization, grants and trust | lib |
| `luna-state` | Persistent state domain, storage abstraction and transactions | lib |
| `luna-event` | Event domain, subscriptions and delivery contracts | lib |

## Bundle and management

| Crate | Responsibility | Form |
|---|---|---|
| `luna-bundle` | Bundle domain, manifest, validation and RFC-0002/LBP1 codec | lib |
| `luna-app-manager` | Application install/import/update/removal/verification/migration | lib + bin where required |
| `luna-system-manager` | System state model and queries | lib + bin where required |
| `luna-update-manager` | State-changing update execution, checkpoints and rollback coordination | lib + bin where required |
| `luna-kernel-manager` | Kernel inventory, metadata and compatibility queries | lib + bin where required |
| `luna-device-manager` | Device and volume discovery/lifecycle | lib + bin where required |

## Runtime / session / login

| Crate | Responsibility | Form |
|---|---|---|
| `luna-system-runtime` | Single system-wide runtime/supervision and UserSession orchestration | lib + bin |
| `luna-user-session` | UserSession domain and lifecycle contract | lib |
| `luna-app-runtime` | ApplicationInstance execution/lifecycle and execution-environment preparation | lib + bin where required |
| `luna-login` | Graphical login integration for the UserSession authentication phase | lib + bin where required |
| `luna-init` | Native musl early-userspace bootstrap; prepares SYSTEM/DATA and enters final root | standalone bin |

`luna-init` is intentionally outside the ordinary userspace workspace: it must remain a minimal early-userspace binary and is built explicitly by the image builders.

There is no separate `luna-session`, `luna-runtime` or `luna-run-session` architecture component.

## User-facing / service-domain clients

| Crate | Responsibility | Form |
|---|---|---|
| `luna-cli` | Thin user-facing CLI client | lib + bin |
| `luna-files` | GTK4 file-manager client/boundary | lib + bin |
| `luna-audio` | Audio domain/provider boundary | lib |
| `luna-network` | Network domain/provider boundary | lib |
| `luna-bluetooth` | Bluetooth domain/provider boundary | lib |

These domain crates do not automatically imply separate daemons. Linux services such as PipeWire, NetworkManager and BlueZ are implementation infrastructure unless an explicit Luna boundary says otherwise.

## Boot

`luna-boot.efi` is a separate UEFI project under `boot/luna-boot/` and is outside the ordinary userspace workspace.

It owns the UEFI boot boundary, image/kernel selection and boot-time fallback/handoff. It does not own UserSessions or application lifecycle.

## Dependency direction

```text
luna-common
   ↑
foundation crates
   ↑
policy/state/bundle/domain crates
   ↑
managers
   ↑
runtime
   ↑
CLI / GUI clients
```

Higher-level components consume lower-level contracts. Lower layers must not depend upward merely for convenience.

## Architectural hierarchy

```text
luna-system-runtime
├── UserSession A
│   ├── luna-app-runtime
│   │   └── ApplicationInstance(s)
│   └── GUI/Desktop session
└── UserSession B
    ├── luna-app-runtime
    │   └── ApplicationInstance(s)
    └── GUI/Desktop session
```

## Important non-boundaries

The following are implementation mechanisms, not Luna architecture components by themselves:

- `setpriv` or similar identity-transition helpers;
- greetd;
- Noctalia Greeter;
- niri-session shell/environment wrapper;
- PipeWire/WirePlumber;
- NetworkManager;
- BlueZ;
- D-Bus;
- Yazi.

They may implement parts of an accepted boundary but must not cause a new Luna component to appear without an accepted architectural decision.

## Current status

All workspace crates above correspond to current repository boundaries. `luna-init` is the deliberate standalone early-userspace exception. Their implementation maturity differs; see the individual component contracts and `docs/architecture/OS-CAPABILITY-GAPS.md`.

RFC-0002 Bundle Format v1 is accepted and `luna-bundle` contains the LBP1 implementation.

The first durable `luna-state` backend is `redb`.

`luna-namespace` contains the first Linux namespace/materialization backend.
