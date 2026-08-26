# Project Luna — Manifest 4 → Phase 1.1 → Phase 1.2 → Phase 1.3 → Phase 1.4

## Authority

- Historical baseline: `docs/architecture/archive/LUNA-MANIFEST-4.md`
- Current authority: `docs/ARCHITECTURE.md`
- Phase 1.1/1.2/1.3/1.4 documents are traceability records.

## Major evolution

### Storage

Manifest 4's earlier DATA layout was replaced by:

```text
DATA/
├── system/{apps,drivers,libs,volumes}/
├── users/<user>/{home,data,config}/
└── cache/
```

SYSTEM and EFI remain hidden from ordinary users.

### Boot and factory

Factory was explicitly clarified as an immutable original System Image + Factory Kernel pair. Boot state is independent metadata. Boot selection is compatibility-aware and supports soft fallback where safe.

### Logical root

Luna constructs a minimal RAM/virtual logical root before DATA is attached. Linux sees a conventional `/`, but DATA does not physically reproduce the Linux directory tree. System Image content is SquashFS and may be loaded lazily.

### Mapping

Mapping became file-oriented, policy-controlled, per-namespace composition with application → user → system precedence. No unrestricted path rewriting and no single global mapping table.

### Application isolation

Each application gets its own Linux-compatible filesystem namespace. Visibility, readability and writability are distinct policy states. Dependencies may map different versions to the same logical path.

### Application lifecycle

Applications are immutable bundles; mutable state lives separately in user DATA. App Manager handles installation/update/removal/verification/migrations and orphaned-data cleanup. Runtime owns execution.

### Component architecture

Phase 1.3 split management and runtime responsibilities into explicit components: app-manager, update-manager, system-manager, kernel-manager, device-manager, root-mapping, security, system-runtime and app-runtime, with thin clients and small daemon+library backends where appropriate.

### Runtime

One system runtime supervises multiple user app runtimes. `ApplicationInstance` supports concurrent/asynchronous execution. Runtime state may be retained in RAM adaptively but is normally released on application close.

### Security

A separate security/policy layer became the central authority. No permanent root user or required sudo/su model. User roles and administrative credentials provide authority.

### Recovery

Recovery became a full working repair environment with a virtual RAM-backed recovery user and no dependency on normal persistent DATA. Factory remains the final known-good original installation state.

### Devices

Connected volumes are automatically exposed through `DATA/system/volumes` and a dedicated Volumes UI. Removable-media autorun is explicitly controlled to avoid silent execution.

### Checkpoints

Btrfs snapshots are a user-visible checkpoint/rollback subsystem, not runtime state and not backups. Scope/retention remain configurable details.

### Phase 1.4

Phase 1.4 further refined shared-library reuse, volume UX, removable-media security, application launch ownership, explicit trust for modified/untrusted metadata, and administrative/user-data protection. Accepted A–T choices are consolidated in the current Source of Truth.

## Classification

- **Superseded:** old DATA root layout and broad early component responsibilities.
- **Refined:** boot, factory, root composition, mappings, namespaces, application lifecycle.
- **New:** separate security layer, runtime split, manager boundaries, user authority model, volume UX and Phase 1.4 trust/admin rules.
- **Open:** detailed wire formats, exact APIs, exact permission schema, exact authentication protocol, exact low-level hybrid SquashFS implementation, and other items explicitly marked open in `docs/ARCHITECTURE.md`.
