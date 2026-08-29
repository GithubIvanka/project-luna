# Project Luna — Post-HZ Architecture Clarifications

**Date:** 2026-08-29
**Status:** ACCEPTED
**Authority:** These decisions are accepted architectural clarifications derived from the Project Luna Source of Truth and the 2026-08-29 repository/history audit. They must be consolidated into `docs/ARCHITECTURE.md` at the next architecture-document maintenance pass.

## 1. Runtime ownership

- `luna-system-runtime` is the single system-wide runtime/supervisor.
- No separate `lunad` architectural component is introduced.
- There is no separate Session Manager.
- `UserSession` is the combined user/session domain entity.
- The runtime hierarchy is:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

- `luna-app-manager` is not part of normal application execution.
- `luna-app-runtime` owns normal application execution and ApplicationInstance lifecycle.

## 2. Logical root and application isolation

- Applications receive a conventional Linux-compatible logical `/` rather than an artificial reduced container filesystem.
- Luna's physical `DATA` layout remains Luna-native and is composed into the logical root through controlled mappings/materialization.
- The application must not be expected to know that its filesystem view is assembled by Luna.
- Linux namespaces, bind mounts and related kernel mechanisms are implementation primitives, not substitutes for Luna's mapping architecture.
- File mappings are the default.
- Explicit subtree/directory mappings are allowed for semantic resource classes such as shared library trees.
- Mapping tables are namespace-specific and RAM-resident at runtime.
- User/application/system precedence remains semantic-class-specific.
- User namespace usage must not be treated as a mechanism for granting ordinary applications root semantics.
- PID/user namespaces may be used for isolation, but exposing an artificial container identity to applications is not a Luna goal.
- `idmapped` mounts are allowed as an implementation primitive when they simplify secure ownership handling; they are not a mandatory Luna abstraction.

## 3. Canonical DATA state layout

The user-visible mutable DATA structure is:

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

- `DATA/system/config/` contains system-wide mutable configuration.
- `DATA/system/state/` contains persistent system state that is not ordinary configuration.
- `DATA/users/<user>/config/` contains user-specific configuration.
- `DATA/users/<user>/data/` contains user/application mutable data.
- `DATA/cache/` remains the common cache area, with semantic separation for system/user/application cleanup where required.

No alternate `DATA/data`, `DATA/apps`, `DATA/portable` or parallel duplicate tree is introduced.

## 4. Security and IPC/device visibility

- `luna-security` remains the central policy authority.
- Security decisions may be `Allow`, `Deny`, `Ask`, or structured constrained access according to the accepted policy model.
- An `Ask` decision is a request for explicit confirmation; Security itself remains UI-agnostic.
- D-Bus access should use a filtered/limited interface rather than expose the unrestricted host system bus to applications.
- `/dev` should be presented as a filtered device view exposing only resources authorized for the application/session.
- USB and external device access follows discovery → policy → authorized access; removable media does not implicitly execute arbitrary software.

## 5. Resource control

- Linux cgroups v2 and related kernel mechanisms are the initial enforcement primitives for CPU, memory and process/resource limits.
- A protected system-critical resource budget is reserved so applications cannot consume all resources and make the OS unresponsive.
- Memory reclamation remains globally controlled by the system rather than by the currently active user.
- Process-count limits and file-descriptor limits are accepted as ordinary resource safeguards.
- Disk/storage usage limits may be enforced where required through filesystem/resource facilities.

## 6. Persistent state implementation direction

- `luna-state` remains a synchronous storage abstraction with revision-checked atomic transactions.
- The first durable backend direction is a small embedded transactional key/value database.
- `redb` is the current implementation choice for this first backend/prototype.
- This backend choice is implementation-level and is not a new architectural boundary.
- A separate custom Luna WAL is not required when the selected backend already provides the durability guarantees required by the state contract.

## 7. Operations, boot success and recovery

- Boot success is not defined solely by kernel handoff. A userspace health/boot-success confirmation is required before a new boot target is considered confirmed.
- A watchdog/timeout can mark an unsuccessful boot attempt when the system fails to reach the required healthy state.
- Repeated application/runtime crashes eventually produce a user-visible recovery/diagnostic decision point according to policy; possible choices include restart, diagnostics, rollback and close.
- Recovery remains a dedicated Recovery System Image with temporary RAM-backed recovery state and a temporary recovery identity.
- Recovery is not Factory and is not a normal persistent user session.

## 8. Bundle and `.lbp`

- `luna-bundle` owns Bundle domain representation and format codec concerns.
- `luna-app-manager` owns install/update/remove/verify/migration/package-import lifecycle.
- No separate `.lbp` parser crate is introduced merely to parse the archive.
- `.lbp` is only the transport/archive representation of a Bundle.
- The installed Bundle is the immutable runtime unit.
- Bundle identity remains BundleId + Version + ContentIdentity.
- Bundle path/location does not define identity.

## 9. Reproducibility and provenance

- Reproducible-build metadata and artifact provenance are accepted as desirable implementation properties of the eventual signature/trust chain.
- Build metadata must not introduce nondeterministic content into ContentIdentity merely through timestamps or local filesystem paths.
- Publisher identity, repository/distribution metadata, content identity and local trust remain separate concepts.

## 10. Documentation and repository rules

- `docs/ARCHITECTURE.md` remains the single Source of Truth.
- Historical phase files preserve traceability and do not compete with the Source of Truth.
- `README.md`, `STATUS.md`, `ROADMAP.md`, `docs/architecture/CRATE-MAP.md` and implementation records must describe actual repository state.
- A stale document must be corrected rather than used as evidence for a new architectural decision.
- Repository implementation must not silently redefine accepted architectural responsibilities.

## 11. Current implementation sequence

```text
1. real Linux namespace + logical-root materialization
2. durable luna-state backend
3. real update/checkpoint/rollback engine
4. final Bundle Format v1 + RFC-0002 acceptance
5. production signature/trust chain
6. System Image/kernel compatibility + boot-state integration
7. final IPC/event transport
8. resource enforcement tuning
9. device/volume integration
10. end-to-end integration testing
```
