# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated through **1.6-HZ**. RFC-0002 Bundle Format v1 was accepted on **2026-08-30**. The project is now in backend integration, end-to-end bring-up and hardening.

All currently accepted architecture decisions are consolidated in `docs/ARCHITECTURE.md`.

## Completed foundation

```text
Architecture consolidation       ← COMPLETED
Repository / Cargo audit         ← COMPLETED
Crate map                        ← COMPLETED / SYNCHRONIZED
Foundation/domain APIs           ← COMPLETED BASELINE
Manager/runtime APIs             ← COMPLETED BASELINE
Integration contracts            ← COMPLETED
Namespace primitive              ← COMPLETED
Logical-root backend             ← IMPLEMENTED
Persistent state abstraction     ← COMPLETED
Durable redb state backend       ← IMPLEMENTED
Update plan abstraction          ← COMPLETED
Checkpoint/apply/verify/rollback ← IMPLEMENTED ENGINE
RFC-0002                         ← ACCEPTED
LBP1 codec                       ← IMPLEMENTED / HARDENING
System runtime process backend   ← IMPLEMENTED
Application process binding      ← IMPLEMENTED PROTOTYPE
Typed runtime contract           ← IMPLEMENTED DEVELOPMENT CONTRACT
QEMU boot/userspace bring-up     ← IMPLEMENTED DEVELOPMENT PATH
luna-boot                        ← WORKING PROTOTYPE → kernel + early userspace + System Image + DATA + shell
```

## Next implementation sequence

### 1. Runtime materialization

The runtime choice is now a typed cross-subsystem contract:

```text
RuntimeKind::Luna  → native Luna / musl
RuntimeKind::Glibc → approved glibc compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

`ApplicationInstance` stores its selected `RuntimeSpec`, mappings can be bound to exactly one runtime kind, and `luna-security` treats runtime access as `Resource::Runtime(kind)` with `Permission::Use`.

Next goals:

- resolve the selected runtime to an approved runtime artifact;
- materialize its loader/library mappings inside the application namespace;
- make glibc runtime versioned and Luna-managed rather than globally visible;
- reject libc/runtime mixing inside one process;
- add runtime compatibility checks without exposing physical runtime paths.

The semantic contract is implemented in `docs/decisions/2026-09-01-RUNTIME-CONTRACT.md`; the final RFC-0002 manifest field remains a separate Bundle decision.

### 2. Security-authorized runtime integration

The first real process/namespace launch path is now connected. Finish the security and resource boundaries around it.

Goals:

- fine-grained authorization for mappings and devices;
- filtered `/dev` population;
- secure physical-path/symlink boundary validation;
- resource-control setup before execution;
- production-safe child creation without relying on post-fork `pre_exec` for complex namespace setup.

### 3. Durable state integration

`luna-state` uses `redb` under `DATA/system/state/luna-state.redb` as the first durable backend.

Goals:

- connect state ownership to `luna-system-runtime` / `luna-system-manager`;
- persist boot/update/runtime state;
- retain revision-checked atomic transactions;
- add integrity/recovery coverage.

### 4. Domain-backed update / checkpoint / rollback engine

`luna-update-manager` is the mutation coordinator; domain managers remain owners of their respective models.

Goals:

- connect real `UpdateBackend` implementations;
- persist exact operation/checkpoint/applied-step state;
- interruption reconciliation;
- explicit rollback;
- System Image/kernel independence;
- application update/migration transactions.

### 5. RFC-0002 implementation conformance

RFC-0002 is now accepted. The remaining work is to make `luna-bundle` a complete, tested reference implementation of the accepted specification.

Goals:

- close the complete parser/writer conformance matrix;
- deterministic canonical payload verification;
- malformed-container fuzz/property tests where useful;
- complete signature-section encoding/verification boundary;
- compatibility tests for future/unknown fields according to the accepted rules;
- installation-stage integration through `luna-app-manager`.

### 6. Production security / signature chain

Implement:

- artifact verification;
- publisher/repository signatures;
- content-identity binding;
- trust records;
- key rotation/revocation;
- runtime permission enforcement;
- `fs-verity` integration where supported.

Signature, trust and permission remain separate concepts.

### 7. System Image + kernel specifications

Formalize:

- per-image manifest;
- image metadata;
- kernel metadata;
- compatibility resolution;
- boot success confirmation;
- persistent boot-state metadata;
- retention policy.

System Images remain direct SquashFS files named `luna-X.Y.Z.squashfs`.

### 8. IPC and event transport

Select the final local IPC/event implementation from the accepted contract: Unix-domain socket control plane with versioned typed protocol, plus the Luna event model. Keep GUI/CLI thin over the backend.

### 9. Resource enforcement

Integrate Linux resource-control mechanisms for CPU, memory, process count, descriptors and useful storage/I/O limits, while reserving protected system-critical resources.

### 10. Device / volume integration

Implement discovery → security policy → authorized access for external devices.

External volumes should appear as friendly entries under:

```text
DATA/system/volumes/<friendly-name>
```

and in the file manager's Volumes view without manual mount commands.

### 11. Desktop System Image

Once the typed runtime contract and runtime materialization are complete, replace the shell-only development System Image with the first real desktop System Image:

```text
UEFI
 ↓
luna-boot
 ↓
Linux kernel
 ↓
luna-init
 ↓
System Image + DATA
 ↓
luna-system-runtime (PID 1)
 ↓
graphical UserSession
 ↓
authentication
 ↓
Wayland
 ↓
niri
 ↓
Noctalia Shell
```

The desktop image remains immutable; mutable desktop/session state belongs in DATA. TTY/serial remains development, diagnostic or recovery-only.

### 12. End-to-end validation

Expand the QEMU path from shell bring-up to a real Bundle/application launch and recovery test:

```text
UEFI
 ↓
luna-boot
 ↓
Linux kernel
 ↓
early userspace
 ↓
System Image + DATA
 ↓
logical root
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
luna-app-runtime
 ↓
RuntimeKind
 ↓
namespace + security
 ↓
ApplicationInstance
 ↓
LBP1 Bundle
```

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current development track reaches the Linux kernel and the early-userspace/System-Image/DATA handoff. The next boot work is hardening and integration, not redesign.

## Non-negotiable constraints

- System Image = direct SquashFS.
- `.lbp` = Bundle transport/archive format.
- `luna-app-manager` does not own normal application process execution.
- `luna-security` remains the central policy authority.
- `luna-root-mapping` remains the mapping layer.
- `luna-namespace` contains Linux-specific namespace/materialization primitives.
- `luna-system-runtime` coordinates system runtime and multiple `UserSession`s.
- `UserSession` is the combined user/session entity.
- Linux namespaces/resource controls are implementation mechanisms for the Luna architecture.
- Accepted decisions are consolidated in `docs/ARCHITECTURE.md` and are not silently changed.
