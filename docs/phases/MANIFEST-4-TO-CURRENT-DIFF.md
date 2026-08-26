# Project Luna — Manifest 4 → Phase 1.1 → Phase 1.2 → Current Architecture Diff

**Baseline:** `LUNA-MANIFEST-4.md`  
**Current authority:** `docs/ARCHITECTURE.md`  
**Review date:** 2026-08-18

## Classification

- **UNCHANGED** — Manifest 4 remains authoritative.
- **UPDATED** — later accepted decision refines or replaces the earlier statement.
- **NEW** — accepted after Manifest 4.
- **OPEN** — discussed, but implementation/specification remains intentionally unresolved.

## 1. Storage layout

**UPDATED.** Manifest 4 used:

```text
DATA/
├── system/
│   ├── apps/
│   └── drivers/
├── users/
├── data/
└── cache/
```

Current:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   └── volumes/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

The old `DATA/data/` directory is superseded.

## 2. EFI / SYSTEM boundary

**UNCHANGED + REFINED.** EFI and SYSTEM remain OS-managed and hidden from ordinary users. SYSTEM contains versioned System Images and kernels. The user-visible mutable storage is DATA.

## 3. System Images

**UNCHANGED.** System Image = directly SquashFS. Each image has a colocated manifest. Images are immutable and independently versioned.

## 4. Kernels

**UNCHANGED + NEW FACTORY DETAIL.** Kernels remain separate from System Images and are selected using image-declared compatibility. At least current and previous non-factory kernels remain for normal fallback. Factory Kernel is a separate immutable installation entity.

## 5. Factory

**UPDATED.** Manifest 4 defined a factory image/kernel pair. Phase 1.2 explicitly clarified that both Factory System Image and Factory Kernel are immutable original installation entities and are never ordinary retention candidates.

## 6. Boot initialization order

**NEW.** Minimal RAM logical root is established before DATA is attached. If DATA cannot be made usable, the system can enter Recovery without depending on normal persistent user DATA.

## 7. Logical Linux root

**NEW / REFINED.** Linux sees a conventional logical `/`, but Luna does not physically recreate the Linux directory zoo in DATA. The root is assembled in RAM/virtual filesystem space and backed by System Image plus controlled DATA mappings.

Hybrid/lazy SquashFS loading remains the accepted direction; exact implementation is OPEN.

## 8. `luna-root`

**NEW.** `luna-root` is narrowly responsible for logical-root construction and mapping composition. It is explicitly not the application manager, session manager, updater or recovery manager.

## 9. Path mapping

**UPDATED.** Mapping is not unrestricted path rewriting. It is policy-controlled composition.

Rules accepted in Phase 1.1/1.2:

- file-oriented mappings;
- no blind whole-directory mapping;
- no arbitrary DATA-to-Linux substitution;
- per-namespace mapping tables;
- explicit mapping classes;
- mapping and permission policy designed together.

## 10. Mapping precedence

**NEW / FORMALIZED.**

```text
application → user → system
```

The most specific mapping wins; lower layers provide fallback.

## 11. Application namespaces

**UPDATED.** Manifest 4 already selected mount namespaces. Phase 1.2 formalized the stronger hybrid model: each application gets its own logical Linux-compatible filesystem view composed from its bundle, dependencies, permitted user/system files, volumes and devices.

Unrelated application files are not visible merely because they exist in DATA.

## 12. Permissions / visibility

**NEW.** Visibility, readability and writability are separate policy states. Applications do not receive unrestricted host filesystem, user, volume or device access.

Exact permission representation remains OPEN.

## 13. Libraries / dependency isolation

**REFINED.** Dependencies are isolated through namespace-local mappings. Example:

```text
App A /lib/gtk → DATA/system/libs/gtk/3
App B /lib/gtk → DATA/system/libs/gtk/4
```

The application sees its normal logical path.

## 14. User filesystem

**NEW.** Each user has `home`, `data`, and `config` under `DATA/users/<user>/`.

Default logical application home:

```text
/home/<user>/
```

mapped to:

```text
DATA/users/<user>/home/
```

Other users' homes are not available by default.

## 15. Configuration precedence

**NEW / FORMALIZED.** User configuration in DATA overrides immutable System Image defaults. Logical `/etc` is a compatibility interface rather than a physical DATA directory.

## 16. Application bundles

**UNCHANGED + REFINED.** Bundles remain immutable units and may be physically moved without moving their mutable user state.

## 17. Application data lifecycle

**NEW.** Application mutable state lives in per-user DATA and survives bundle movement/removal according to policy. App Manager is responsible for finding orphaned data and providing cleanup controls.

## 18. Namespace state retention

**UPDATED.** Earlier discussion used roughly one hour as an example. Current decision is configurable + adaptive memory-pressure eviction. Mapping tables remain in RAM, not persistent DATA.

## 19. Multiple users

**NEW.** Multiple users can remain active simultaneously.

## 20. Session switching

**NEW.** Each user independently chooses:

1. continue applications normally;
2. keep them alive but restricted;
3. terminate them.

Default: 2.

## 21. Applying configuration changes

**NEW.** Re-entering/restarting the affected session or service is preferred over a full reboot where sufficient. Long-running system services such as the updater may continue across user changes when safe.

## 22. Recovery

**UPDATED.** Recovery is not just a shell. It is a functional Luna environment without normal persistent DATA, using a temporary virtual recovery user in RAM and providing diagnostic/repair/data-recovery tools.

## 23. Failure handling

**NEW.** Luna prefers diagnosis → repair → notification → explicit emergency state rather than an immediate dead-end panic screen.

## 24. Btrfs

**UPDATED.** Btrfs snapshots are a checkpoint/rollback subsystem for selected mutable DATA state. They are not runtime session-switching and not a long-term backup mechanism.

The user can choose targeted option 2, broader option 3, or disable the subsystem. Default: option 2.

## 25. SYSTEM write protection

**NEW / FORMALIZED.** SYSTEM has two layers: filesystem read-only state plus authorization restricting writes to the updater/system-management component.

## 26. Dependency downloads

**NEW.** Missing dependencies are not silently downloaded. Luna identifies the requirement, finds a suitable source if possible, explains it and asks the user before installation.

## 27. Application Manager boundaries

**NEW.** App Manager owns install, launch, update, remove and application-data lifecycle. Runtime owns process/namespace lifecycle. `luna-root` owns logical root and mappings.

## 28. Explicitly unchanged invariants

These remain from Manifest 4:

- Rust;
- Apache 2.0;
- EFI / SYSTEM / DATA / SWAP physical model;
- custom `luna-boot.efi`;
- `B` boot menu;
- System Images in SYSTEM;
- SquashFS System Images;
- per-image manifests beside images;
- independent kernels;
- manifest-defined kernel compatibility;
- current/factory concepts;
- soft fallback;
- `.lbp` Bundle Format separate from System Images;
- mount namespaces;
- automatic external-storage UX;
- niri + Noctalia Shell;
- Ghostty + fish;
- design-before-code workflow.

## 29. Explicitly still OPEN

The following were deliberately not promoted into implementation specifications:

- exact `luna-root` API/process split;
- exact Linux mount API sequence;
- exact SquashFS lazy-loading implementation;
- exact mapping-table representation;
- exact mapping path taxonomy;
- exact read/write rules for every path class;
- exact `/var` decomposition;
- exact namespace persistence implementation;
- exact permission policy language/enforcement;
- exact device permission API;
- exact restricted-session behavior;
- exact memory-pressure eviction algorithm;
- exact dependency discovery/repository protocol;
- exact recovery boot protocol;
- exact emergency state;
- exact checkpoint scope/retention/creation rules;
- exact updater transaction semantics across user switches.

## Final conclusion

Manifest 4 remains the historical baseline, not the final current architecture. Phase 1.1 and Phase 1.2 did not overturn Luna's core philosophy; they made the runtime/data boundary significantly more precise.

The largest architectural evolution is the move from “Linux-compatible filesystem stored in a clean layout” to a precise model of:

```text
physical Luna storage
        ↓
logical RAM root
        ↓
per-namespace file mappings
        ↓
visibility + permissions
        ↓
application-specific Linux filesystem view
```

That model is now the foundation for the next architecture phase.
