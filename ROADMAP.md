# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated through **1.6-HZ**. The project has moved from contract-only work into real Linux backend implementation.

## Completed foundation

```text
Architecture consolidation       ← COMPLETED
Repository / Cargo audit         ← COMPLETED
Crate map                        ← COMPLETED / SYNCHRONIZED
Foundation/domain APIs           ← COMPLETED BASELINE
Manager/runtime APIs             ← COMPLETED BASELINE
Integration contract prototypes  ← COMPLETED
Namespace primitive              ← COMPLETED
Logical-root materialization     ← IMPLEMENTED BACKEND
Persistent state abstraction     ← COMPLETED
Durable redb state backend       ← IMPLEMENTED
Update plan abstraction          ← COMPLETED
Checkpoint/apply/verify/rollback ← IMPLEMENTED ENGINE
luna-boot                        ← WORKING PARALLEL PROTOTYPE → kernel + test init + sh
```

## Next implementation sequence

### 1. Security-authorized runtime integration

Connect the real Linux namespace/logical-root backend to the existing Luna policy and runtime boundaries.

Goals:

- `luna-security` authorization before writable/device mappings;
- `luna-app-runtime` child-process creation;
- conventional logical `/` without exposing the host filesystem;
- filtered `/dev` population from authorized resources;
- secure path/symlink boundary validation;
- resource-control integration.

### 2. Durable state integration

The first durable backend is now `redb` under `DATA/system/state/luna-state.redb`.

Goals:

- connect state ownership to system/runtime managers;
- persist boot/update/runtime state;
- retain revision-checked atomic transactions;
- add recovery/integrity integration where needed.

### 3. Domain-backed update / checkpoint / rollback engine

`luna-update-manager` is the mutation coordinator, not the owner of application/system/kernel lifecycle.

Goals:

- connect `UpdateBackend` to domain managers;
- prepare;
- checkpoint;
- apply;
- verify;
- commit;
- interruption reconciliation;
- explicit rollback;
- System Image/kernel independence;
- application update/migration transactions.

### 4. Bundle Format v1

Design and formally accept RFC-0002, then implement `.lbp` in `luna-bundle`.

`luna-app-manager` owns lifecycle operations and package import; `luna-bundle` owns Bundle representation and format codec concerns.

### 5. Production security / signature chain

Implement:

- artifact verification;
- publisher/repository signatures;
- content identity binding;
- trust records;
- signature failure handling;
- runtime permission enforcement.

Signature, trust and permission remain separate concepts.

### 6. System Image + kernel specifications

Formalize:

- per-image manifest;
- image metadata;
- kernel metadata;
- compatibility resolution;
- boot success confirmation;
- persistent boot-state metadata.

### 7. IPC and event transport

Select the final local IPC transport and production event delivery mechanism only after backend contracts are exercised. Keep GUI/CLI thin over shared backend APIs.

### 8. Resource enforcement

Integrate Linux resource-control mechanisms for CPU, memory, process count, file descriptors and useful storage/I/O limits, while reserving protected system-critical resources.

### 9. Device / volume integration

Implement discovery → policy → authorized access for external devices. External volumes should appear in `DATA/system/volumes/<friendly-name>` and the file manager's Volumes view without manual mount commands.

### 10. End-to-end validation

Add reproducible CI/integration paths covering:

```text
UEFI
 ↓
luna-boot
 ↓
Linux kernel
 ↓
System Image
 ↓
logical root
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
luna-app-runtime
 ↓
ApplicationInstance
```

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track already reaches the kernel plus a test init path ending at `sh`. Production signature/trust, boot-success confirmation, persistent boot-state metadata and remaining compatibility/hardening work remain on the roadmap.

## Non-negotiable constraints

- System Image = direct SquashFS, not `.lbp`.
- `.lbp` = Bundle transport/archive format, not System Image.
- SYSTEM and DATA remain separate.
- `luna-app-manager` does not own normal application execution.
- `luna-security` remains the central policy authority.
- `luna-root-mapping` remains a narrow mapping layer.
- `luna-fs` remains a low-level filesystem abstraction.
- One `luna-system-runtime` coordinates multiple `UserSession`s.
- `UserSession` is the combined user/session entity.
- There is no separate `lunad` architecture component.
- Linux namespaces/resource controls are implementation mechanisms, not the architecture itself.
- Accepted decisions are not silently changed.
