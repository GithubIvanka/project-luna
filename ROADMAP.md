# Project Luna — Roadmap

The architectural Source of Truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This roadmap describes sequence and dependencies, not deadlines.

## Current position

Phases **1.1–1.6** are accepted/consolidated through **1.6-HZ**. RFC-0002 Bundle Format v1 was accepted on **2026-08-30**. The project is now in backend integration and hardening.

Post-1.6 accepted clarifications are recorded in `docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md` until they are safely consolidated into the large Source of Truth document.

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
luna-boot                        ← WORKING PARALLEL PROTOTYPE → kernel + test init + sh
```

## Next implementation sequence

### 1. Security-authorized runtime integration

Connect the Linux namespace/logical-root backend to the existing Luna security and runtime boundaries.

Goals:

- authorization before writable/device mappings;
- real child-process creation and supervision;
- conventional logical `/` without exposing the host filesystem;
- filtered `/dev` population from authorized resources;
- secure physical-path/symlink boundary validation;
- resource-control setup before execution.

### 2. Durable state integration

`luna-state` uses `redb` under `DATA/system/state/luna-state.redb` as the first durable backend.

Goals:

- connect state ownership to `luna-system-runtime` / `luna-system-manager`;
- persist boot/update/runtime state;
- retain revision-checked atomic transactions;
- add integrity/recovery coverage.

### 3. Domain-backed update / checkpoint / rollback engine

`luna-update-manager` is the mutation coordinator; domain managers remain owners of their respective models.

Goals:

- connect real `UpdateBackend` implementations;
- persist exact operation/checkpoint/applied-step state;
- interruption reconciliation;
- explicit rollback;
- System Image/kernel independence;
- application update/migration transactions.

### 4. RFC-0002 implementation conformance

RFC-0002 is now accepted. The remaining work is to make `luna-bundle` a complete, tested reference implementation of the accepted specification.

Goals:

- close the complete parser/writer conformance matrix;
- deterministic canonical payload verification;
- malformed-container fuzz/property tests where useful;
- complete signature-section encoding/verification boundary;
- compatibility tests for future/unknown fields according to the accepted rules;
- installation-stage integration through `luna-app-manager`.

### 5. Production security / signature chain

Implement:

- artifact verification;
- publisher/repository signatures;
- content-identity binding;
- trust records;
- key rotation/revocation;
- runtime permission enforcement;
- `fs-verity` integration where supported.

Signature, trust and permission remain separate concepts.

### 6. System Image + kernel specifications

Formalize:

- per-image manifest;
- image metadata;
- kernel metadata;
- compatibility resolution;
- boot success confirmation;
- persistent boot-state metadata;
- retention policy.

System Images remain direct SquashFS files named `luna-X.Y.Z.squashfs`.

### 7. IPC and event transport

Select the final local IPC/event implementation from the accepted contract: Unix-domain socket control plane with versioned typed protocol, plus the lightweight Luna event model. Keep GUI/CLI thin over the backend.

### 8. Resource enforcement

Integrate Linux resource-control mechanisms for CPU, memory, process count, descriptors and useful storage/I/O limits, while reserving protected system-critical resources.

### 9. Device / volume integration

Implement discovery → security policy → authorized access for external devices.

External volumes should appear as friendly entries under:

```text
DATA/system/volumes/<friendly-name>
```

and in the file manager's Volumes view without manual mount commands.

### 10. End-to-end validation

Add reproducible Linux/QEMU integration paths covering:

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

`luna-boot.efi` is maintained separately under `boot/luna-boot/`. The current boot track reaches the kernel plus a test init path ending at `sh`. Bootloader redesign is not part of the current Bundle implementation sequence.

## Non-negotiable constraints

- System Image = direct SquashFS, not `.lbp`.
- System Images live in `SYSTEM`, never in DATA.
- `.lbp` = Bundle transport/archive format, not System Image.
- `luna-app-manager` does not own normal application execution.
- `luna-security` remains the central policy authority.
- `luna-root-mapping` remains a narrow mapping layer.
- `luna-namespace` contains Linux-specific namespace/materialization primitives.
- One `luna-system-runtime` coordinates multiple `UserSession`s.
- `UserSession` is the combined user/session entity.
- There is no separate `lunad` architecture component.
- Linux namespaces/resource controls are implementation mechanisms, not the architecture itself.
- Accepted decisions are not silently changed.
