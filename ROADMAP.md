# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated, with Phase 1.6 complete through **1.6-HZ**. The project has passed the initial repository/API scaffold stage and is now entering real backend implementation.

## Completed foundation

```text
Architecture consolidation       ← COMPLETED
Repository / Cargo audit         ← COMPLETED
Crate map                        ← COMPLETED / SYNCHRONIZED
Foundation/domain APIs           ← COMPLETED BASELINE
Manager/runtime APIs             ← COMPLETED BASELINE
Integration contract prototypes  ← COMPLETED
Namespace/materialization        ← CONTRACT PROTOTYPE
Persistence/update               ← IN-MEMORY ATOMIC PROTOTYPE
luna-boot                        ← WORKING PARALLEL PROTOTYPE → kernel + test init + sh
```

## Next implementation sequence

### 1. Real Linux namespace + logical-root materialization

Implement the actual backend behind the existing mapping contracts.

Goals:

- mount namespace creation;
- controlled bind/subtree mounts where required;
- logical Linux-compatible `/`;
- per-namespace mapping application;
- secure path/symlink boundary validation;
- no artificial container filesystem exposed to applications.

### 2. Durable persistent state

Implement the backend behind `luna-state` while preserving its synchronous, transactional API.

Goals:

- durable state under `DATA/system/state/`;
- revision-checked atomic transactions;
- crash-safe recovery;
- minimal storage overhead;
- no separate custom WAL when the selected backend already provides required durability.

### 3. Real update / checkpoint / rollback engine

`luna-update-manager` remains the mutation executor.

Goals:

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

Design and formally accept RFC-0002, then implement `.lbp`.

`luna-bundle` owns Bundle representation/format concerns; `luna-app-manager` owns lifecycle operations and package import.

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

Integrate Linux resource-control mechanisms for:

- CPU;
- memory;
- process count;
- file descriptors;
- I/O/disk constraints where useful;
- protected system resource budget.

### 9. Device / volume integration

Implement automount and safe-removal behavior so external volumes appear in `DATA/system/volumes/<friendly-name>` and the file manager's Volumes view without manual mount commands.

### 10. End-to-end validation

Add reproducible CI/integration paths covering the real sequence:

```text
UEFI
 ↓
luna-boot
 ↓
Linux kernel
 ↓
logical root
 ↓
system-runtime
 ↓
UserSession
 ↓
app-runtime
 ↓
ApplicationInstance
```

## Bootloader status

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track has already progressed beyond the initial scaffold and reaches the kernel plus a test init path ending at `sh`. Production signature/trust, final boot-state metadata and remaining compatibility/hardening work remain on the roadmap.

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
