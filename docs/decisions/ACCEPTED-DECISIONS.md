# Project Luna — Accepted Architecture Decisions

**Status:** canonical accepted-decision ledger  
**Current through:** 2026-08-31  
**Authoritative current architecture:** `docs/ARCHITECTURE.md`  
**Purpose:** one compact record of the accepted outcomes from the architecture discussion. Historical phase files and ADRs remain traceability records; they do not override this ledger or the Source of Truth.

## 1. Project foundations

- Project name: Project Luna; internal name `luna`.
- Rust is the implementation language.
- Apache License 2.0.
- Linux kernel is the kernel foundation.
- One File Linux is an architectural inspiration, not an implementation constraint.
- The system foundation is intentionally small, stable and predominantly immutable.
- Existing Linux mechanisms are preferred where they provide a sound primitive; Luna defines its own higher-level architecture around them.
- New crates are introduced only when they create a real responsibility boundary.
- `luna-common` remains deliberately small and is not a dumping ground.
- Tokio is the accepted async runtime for components that actually need asynchronous execution; not every crate must depend on Tokio.

## 2. Physical storage model

The installation is divided into:

```text
EFI
SYSTEM
DATA
SWAP
```

- `EFI` contains boot infrastructure and is OS-managed.
- `SYSTEM` contains versioned immutable Luna System Images and kernels.
- `DATA` contains mutable machine, user, application and cache state.
- `SWAP` is optional and may be implemented as a partition, file or ZRAM.
- EFI and SYSTEM are not ordinary user-managed filesystem areas.
- EFI/SYSTEM and DATA/SWAP may live on different physical disks.

Canonical DATA layout:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`DATA/system/state` is the persistent system-state location. `DATA/cache` is disposable and independently manageable; system, user and application cache may be cleaned separately.

## 3. System Images

- A System Image is directly a SquashFS filesystem image.
- Canonical name: `luna-X.Y.Z.squashfs`.
- Each System Image has its own manifest beside it: `luna-X.Y.Z.toml`.
- System Images live on SYSTEM.
- System Images are immutable during normal operation.
- There is no global System Image manifest.
- System Image version and kernel version are independent.
- Compatible image/kernel combinations are determined by metadata and compatibility rules.
- System Image content may be exposed lazily/hybrid rather than copied completely into RAM at boot.
- Factory is the original installed, known-good System Image plus its factory kernel.
- Factory remains preserved and immutable.
- System Images have retention policies; the current and previous usable states must remain available according to policy.

## 4. Kernel model

- Kernels are independent versioned entities on SYSTEM.
- Kernel install/update/removal belongs to the update path with kernel-domain validation supplied by `luna-kernel-manager`.
- The current kernel is never removed by ordinary cleanup.
- At least the current and a usable previous kernel are retained according to policy.
- Kernel selection always respects System Image compatibility.
- Factory retains a known-good factory kernel.
- Kernel and System Image updates are independently executable.

## 5. Boot architecture

- `luna-boot.efi` is a separate boot-specific component outside the normal userspace workspace.
- Normal boot is quiet and does not wait for a persistent menu.
- Pressing `B` during the boot window opens the boot menu.
- The normal boot path is UEFI → `luna-boot.efi` → compatible kernel → Luna System Image → RAM/logical root → `luna-system-runtime` → UserSession(s).
- Boot selection uses System Image manifests and kernel compatibility metadata.
- Boot state is separate from System State and Recovery State.
- Boot state changes only on relevant events, not on every ordinary boot.
- Soft fallback tries compatible previous system/kernel choices before factory recovery where possible.
- If the current image fails and a compatible previous image can be used with the current kernel, it may be tried without a full reboot.
- If kernel compatibility requires another kernel, reboot may select another compatible kernel and image.
- Recovery and Factory are distinct modes.
- Emergency behavior is a failure/recovery mode, not a replacement for the normal boot menu.
- `luna-boot` has already demonstrated kernel loading through the test init and shell (`sh`) in the dedicated boot work.

## 6. Recovery and Factory

- Factory is the original normal system state: factory System Image + factory kernel + normal DATA.
- Recovery is a separate recovery System Image.
- Recovery uses a temporary recovery identity with writable state in RAM.
- Recovery can operate when normal DATA is unavailable.
- Recovery contains diagnostic/repair utilities and the minimum environment needed to repair or restore the installed system.
- Recovery access to protected user DATA requires explicit authorization/password as defined by the security model.
- Recovery state disappears on reboot unless explicitly persisted by a future design.
- Factory remains available as the last known-good system state.

## 7. Logical Linux root

- Applications see a conventional Linux-compatible logical `/`.
- The physical Luna storage layout is not exposed as the application's filesystem tree.
- The logical root is assembled from controlled sources, including System Image content, approved DATA mappings, application Bundle resources, user state and approved volumes/devices.
- Physical `SYSTEM/...` and `DATA/...` paths are implementation details.
- Applications must not need to know they are running inside a container-like mechanism.
- Mapping is a Linux-compatible composition layer, not a physical copy of the DATA hierarchy.
- System files may be loaded into RAM on demand; if a currently active System Image is removed after its required content has been materialized, system content already loaded into RAM must not be reclaimed when no other source exists.

## 8. Root mapping

- The component name is `luna-root-mapping`.
- It owns logical mapping semantics and validated mapping plans.
- Mapping plans are built from manifest/resource declarations and runtime/user/system context.
- Mapping tables live in RAM for the active application/security context.
- Identical immutable mapping definitions may be shared between application instances when safe; instance-specific state remains independent.
- File mappings are the default granularity.
- Explicit subtree/directory mappings are allowed where semantically appropriate, especially shared libraries/resources.
- Mapping semantics are semantic-class-specific; there is no universal filesystem precedence rule.
- The usual layered lookup is user → application → system where that resource class supports those layers.
- Conflicts inside one namespace are errors; silent overwrite is not a valid mapping behavior.
- Physical path details remain internal.
- A mapping declaration is not itself a security grant.
- An active ApplicationInstance cannot mutate its accepted mapping table in place.
- A change produces a new validated mapping state and may require policy revalidation.

## 9. Security and permissions

- `luna-security` is the central policy authority.
- Security policy is revisioned.
- Authorization decisions may be bound to a policy revision/snapshot.
- Permission dimensions include Visibility, Read, Write, Execute, Device Use and Manage.
- Application-wide restrictions apply to all instances of the relevant application identity.
- An individual instance may tighten restrictions but may not weaken an enforced deny.
- `Ask` means explicit confirmation is required; security remains UI-agnostic.
- `Constrained` carries structured typed restrictions rather than arbitrary strings.
- Grants may be one-time, operation-scoped, while-running or persistent as appropriate.
- Bundle manifest mappings/capabilities/access declarations are requests, not grants.
- Trust is content-specific and binds at least BundleId, ContentIdentity and trust scope.
- Signature validity, trust and authorization remain separate decisions.
- The policy enforcement chain is declaration → mapping → security → namespace/runtime enforcement.
- A system-level policy may impose a restriction even when an application manifest requests the corresponding mapping.
- Administrator privileges are tied to explicit administrative authority; there is no permanent root user or mandatory `sudo`/`su` layer.
- An administrator account cannot be left with an empty administrator password when the system permits downgrading it to an ordinary user.
- The system administrator password may be used to regain administrative authority, and a future recovery-key/password-recovery mechanism is part of the accepted design direction.
- User data may be unencrypted by default, but future installation/user settings may enable encryption for whole DATA and/or individual user data.

## 10. Application model

- Applications are immutable macOS-like Bundles.
- Installed Bundles are stored under `DATA/system/apps`.
- A Bundle is shared between users rather than copied for every user.
- Different application versions are independent and may coexist.
- An application can be portable and launched from another disk/removable media after inspection and trust/authorization checks.
- A Bundle may be registered into the normal Apps view through managed registration/linking without changing its identity.
- Application mutable state is outside the immutable Bundle.
- User application data belongs under `DATA/users/<user>/data`.
- User application configuration belongs under `DATA/users/<user>/config`.
- Default configuration may live in the immutable Bundle and be overridden by user data/config layers.
- Removing a Bundle does not automatically require removing its user data; manual removal may ask the user, while automatic cleanup follows retention/lock policy.
- Orphaned application data is discoverable by `luna-app-manager`; the user may remove it deliberately.
- Data can be marked retained/locked against automatic cleanup.
- By default, closing an application releases its runtime resources; an explicit per-application/user policy may retain a warm instance/cache when desired.

## 11. Application runtime hierarchy

The system runtime hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   └── luna-app-runtime
│       └── ApplicationInstance(s)
└── UserSession B
    └── luna-app-runtime
        └── ApplicationInstance(s)
```

- `luna-system-runtime` is the single system-wide runtime/supervisor.
- `UserSession` is the combined user/session entity.
- `luna-app-runtime` owns ApplicationInstance lifecycle and execution setup.
- `luna-app-manager` does not own normal application process execution.
- ApplicationInstance identity is separate from Bundle identity.
- `ApplicationInstanceId` is system-wide unique and managed at system-runtime level.
- Multiple instances of the same Bundle may exist when policy permits.
- Instance-level policy may only tighten application policy.
- If an app-runtime fails, system-runtime may restart it without automatically destroying the UserSession.
- Recovered runtime metadata does not imply restoration of the application's in-memory process state.

## 12. UserSession

- User and session are a single domain object: `UserSession`.
- Multiple UserSessions may exist concurrently.
- UserSession state includes user identity, session state and relevant policy/resource context.
- Accepted session states include ACTIVE, RESTRICTED and TERMINATED.
- Default behavior when a user leaves the active desktop session is RESTRICTED.
- Each user/session may independently configure behavior for applications: continue, remain managed/restricted, or terminate.
- System services and update operations may continue when the active user changes.
- A user switch does not require system-wide state recreation.

## 13. Filesystem, devices and external volumes

- `luna-fs` owns low-level filesystem primitives and errors.
- `luna-namespace` owns Linux-specific namespace/materialization primitives.
- Applications receive a controlled `/dev` view rather than unrestricted host device access.
- Device use is permission-controlled.
- `luna-device-manager` owns device discovery, volume lifecycle and automount orchestration.
- External volumes appear automatically in the file manager with friendly names.
- Managed volume state is represented under `DATA/system/volumes`.
- The file manager exposes the Luna DATA structure in its user-facing model.
- A USB volume may be read/written according to permissions and configuration without manual mount commands.
- USB autorun is disabled or confirmation-based according to system policy; connecting media does not silently execute an application.
- Network volumes may use the same volume abstraction in a future extension.

## 14. Linux namespace model

- Each ApplicationInstance receives an isolated filesystem/mount namespace.
- Linux namespaces are implementation primitives, not the user-facing architecture.
- Mount namespace isolation is mandatory for application filesystem separation.
- User namespace, PID namespace, network namespace, IPC/UTS/time isolation and other primitives are policy-driven according to the application's resource/security profile.
- Application isolation must preserve the appearance of a conventional Linux filesystem rather than expose container implementation details.
- The initial implementation uses Linux mount namespaces, controlled bind mounts and OverlayFS where useful.
- `luna-namespace` may use idmapped mounts when they simplify UID/GID presentation without copying or chowning files.
- No application receives `CAP_SYS_ADMIN` or equivalent host-level privilege by default.
- PID namespace use is for isolation and supervision; it is not required to make the application believe it is PID 1.
- `cgroups v2` is the accepted resource-control primitive.

## 15. Resource protection

- The system reserves protected CPU, memory and GPU capacity for system/runtime/diagnostics functions.
- Reservation is adaptive rather than one hard-coded percentage for every machine.
- Linux resource-control mechanisms perform the low-level enforcement.
- Memory-pressure reclamation proceeds from disposable/reclaimable data toward application pressure and eventual termination, while protected system-critical resources remain available.
- CPU and memory policies are configurable per application/user policy.
- Process-count limits protect against runaway process creation.
- File-descriptor limits protect against exhaustion of kernel-managed handles.
- Persistent DATA may have quotas at a later policy layer.

## 16. Configuration and state

- TOML is the preferred human-readable configuration/metadata format where appropriate.
- Machine-wide mutable configuration lives in `DATA/system/config`.
- User-scoped configuration lives in `DATA/users/<user>/config`.
- Application defaults are immutable in the Bundle/System Image as appropriate.
- Configuration precedence is semantic-class-specific; the common user → application/default → system-default model is used where applicable.
- System-wide configuration must survive switching users.
- `luna-state` owns the persistent logical state abstraction and revision model.
- Persistent system state is stored under `DATA/system/state`.
- The first durable backend is `redb`.
- State transactions are atomic and use revision-based optimistic concurrency.
- State persistence is separate from checkpoint/rollback storage.

## 17. Events and operations

- Event and Operation identities are separate.
- Events have monotonic sequence within an operation where an operation exists; timestamps are metadata, not ordering.
- Event classes are Ephemeral, Persistent and Audit.
- Persistent/Audit events support history/replay as appropriate.
- Delivery is bounded and backpressure-aware.
- Audit events are never silently dropped.
- Operations may be interrupted and reconciled after runtime/service recovery.
- Operations declare whether they are resumable, non-resumable or otherwise require reconciliation.
- Operations belong to System or UserSession context, not GUI/CLI process lifetime.
- Operation authorization distinguishes viewing, canceling, resuming and rollback.
- Cooperative cancel and force stop are distinct; force stop is stronger, warning-bearing and audited.
- GUI/CLI disconnection does not silently cancel backend work.

## 18. Managers

- `luna-app-manager`: install, import, update, remove, verify, migrate, inspect and manage application data.
- `luna-app-manager` can ingest `.deb` and `.rpm` packages by analyzing their Linux filesystem layout, constructing a Luna Bundle and generating its manifest/mapping description.
- `luna-system-manager`: owns the system state model/query layer.
- `luna-kernel-manager`: owns kernel inventory, metadata and compatibility queries.
- `luna-device-manager`: owns device/volume discovery and lifecycle.
- `luna-update-manager`: executes state-changing update transactions across system/application/kernel domains.
- Domain managers own the model of their own domain; update-manager owns execution of mutations.
- `luna-system-runtime` owns runtime state/supervision rather than replacing all managers with one monolithic daemon.

## 19. Update and rollback

- Update stages are prepare → checkpoint → apply → verify → commit.
- Updates are transactional and recoverable.
- The previous authoritative state remains available until commit is confirmed where possible.
- Interrupted transactions are reconciled from durable operation state.
- Rollback is explicit and user-visible; automatic rollback may be policy-triggered when a health/boot contract requires it.
- A failed update may present user choices such as cancel, retry, diagnose, rollback or recovery.
- System Images use versioned filenames/metadata rather than a separate generation number as the primary user-facing version model.
- Different application versions remain independent; old versions can remain available for compatibility after a major update.
- Minor/patch versions may be cleaned according to retention after the updated application is confirmed healthy.
- Delta updates are supported as a future/update transport mechanism and are separate from the `.lbp` container specification.
- Removing the currently running System Image requires materializing the required runtime content into RAM first and confirming that the system no longer depends on the deleted image.
- Checkpoints are separate from the logical state database.
- Btrfs snapshots are the accepted checkpoint implementation direction for persistent rollback where applicable.

## 20. Bundle and dependency model

- `luna-bundle` owns Bundle representation, manifest model, validation and RFC-0002 `.lbp` codec.
- `.lbp` is the transport/archive representation of a Bundle.
- Installed Bundle state is immutable.
- Bundle identity is `BundleId + Version + ContentIdentity`.
- ContentIdentity is independent of filename and physical location.
- ContentIdentity uses BLAKE3-256 over canonical Bundle content.
- The accepted RFC-0002 v1 container uses a fixed 64-byte header and fixed 64-byte section entries, little-endian integers and BLAKE3 integrity hashes.
- The required sections are MANIFEST and PAYLOAD; RESOURCES and SIGNATURE are optional.
- MANIFEST is canonical TOML.
- PAYLOAD is deterministic TAR, with canonical metadata and zstd as the canonical compression policy.
- Symlinks, hard links and special device/FIFO/socket entries are excluded from v1 payloads.
- Bundle mapping paths are Linux-style absolute logical paths; Bundle source paths are validated relative paths.
- Bundle manifests may request capabilities and access modes; they do not grant them.
- Dependency declarations use supported SemVer constraints; unsupported grammar is rejected.
- `dependi` is build/install/dependency-resolution tooling, not part of runtime execution.
- Dependency resolution is deterministic.
- Delta update transport is outside RFC-0002.

## 21. LBP1 security and authenticity

- RFC-0002 v1 is Accepted.
- Unknown major format versions are rejected.
- Unknown mandatory flags are rejected.
- Invalid section ranges, overlaps, truncation, overflow and unsafe paths are rejected.
- Structural validation and integrity validation happen before extraction.
- Signature validity is separate from trust and authorization.
- Ed25519 is the accepted Bundle signature algorithm.
- A signature covers canonical Bundle content/metadata while excluding the signature section itself.
- Publisher and repository authenticity may both participate in the broader supply-chain model.
- Unsigned Bundles are format-valid; whether they are installable/launchable depends on security/trust policy.
- Key rotation, revocation and trusted-key management are part of the production security architecture.
- `fs-verity` is an accepted Linux integrity primitive for immutable Bundle/system content where supported.
- IMA integration is accepted as a future/production hardening layer.
- TPM measured boot is optional future hardening, not required for the initial implementation.

## 22. Communication and clients

- `luna-cli` is the main CLI client.
- CLI syntax uses short aliases over the same backend operations, such as `app install`, `app -i`, `sys update`, `sys -u`, `dev list`; exact final syntax is a later CLI specification detail.
- User aliases are configurable, with descriptions so the user can understand the mapping.
- GUI and CLI are thin clients over the same backend contracts.
- Backend operations remain independent of client lifetime.
- Internal component APIs are versioned; breaking changes require a major API version change and incompatible clients are rejected.
- Unix socket IPC with a small versioned structured/binary protocol is the accepted direction.
- Machine-readable CLI output is supported in addition to human-readable output.
- D-Bus, where used, is exposed through filtered/limited interfaces rather than unrestricted host-bus access.
- Wayland is the accepted display integration direction.
- PipeWire is the accepted audio/media backend.

## 23. Async/concurrency model

- Luna is intended to be multicore/multithreaded and asynchronous where useful for responsiveness.
- Tokio is the first async runtime direction where asynchronous execution is needed.
- Storage abstractions remain synchronous unless a specific backend requires otherwise; async orchestration belongs above them.
- Shared immutable plans/tables may be reused where safe; mutable runtime state has explicit ownership.
- System-level supervision covers multiple UserSessions and their application runtimes.

## 24. Desktop and user environment

- niri is the chosen compositor/window manager direction.
- Noctalia Shell is the chosen shell/UI direction.
- Ghostty + fish is the chosen terminal environment.
- Desktop components remain outside the core architecture.

## 25. Rust development rules

- Code should be written to be understandable to a developer learning Rust.
- Important ownership/borrowing decisions are explained.
- `struct`, `enum`, `trait`, `Result`, `Option`, generics and lifetimes are explained when materially relevant.
- Crate boundaries and module boundaries are explained.
- Rust solutions should prefer clear, simple designs over clever abstraction when both satisfy the architecture.
- Existing code is source material, not an authority over the architecture.

## 26. Source-of-truth and change control

- `docs/ARCHITECTURE.md` is the current architectural Source of Truth.
- `docs/decisions/ACCEPTED-DECISIONS.md` is the single compact ledger of accepted outcomes.
- Phase files preserve development history and decision context.
- RFC files define normative formats/protocols where applicable.
- Archive files preserve superseded historical material.
- A new proposal does not silently replace an accepted decision.
- When implementation reveals a real architectural conflict, it must be reported explicitly and resolved before changing the SoT.
- New chats must read the current SoT before proposing architecture.

## 27. Current accepted phase status

```text
Phase 1.1 — accepted
Phase 1.2 — accepted
Phase 1.3 — accepted
Phase 1.4 — accepted
Phase 1.5 — accepted
Phase 1.6 — accepted through the HZ decision set and subsequent accepted clarifications
RFC-0002 Bundle Format v1 — accepted
```
