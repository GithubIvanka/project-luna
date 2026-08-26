# Project Luna — Architecture Decision History

**Purpose:** preserve important decisions, corrections, rejected directions and unresolved questions from the main architecture discussion so future chats do not silently lose context.

## Status vocabulary

- **ACCEPTED** — explicitly accepted by the user.
- **WORKING** — accepted direction, but implementation details remain open.
- **PROPOSAL** — assistant/user idea not yet accepted as a specification.
- **SUPERSEDED** — older decision replaced by a later explicit decision.
- **OPEN** — intentionally unresolved.

## 1. Project identity

**ACCEPTED**
- Project name: Project Luna.
- Internal short name: `luna`.
- Rust is the implementation language.
- Apache License 2.0.
- Linux kernel is used as the kernel.
- One File Linux is an architectural inspiration, not a requirement to copy its implementation.

## 2. Core architectural philosophy

**ACCEPTED**
- Very small immutable/stable system foundation.
- System Images are immutable.
- System updates and kernel updates are independent.
- User/application data must not be able to corrupt the immutable OS image.
- Luna should present a clean, simple storage model instead of exposing the traditional Linux directory zoo.

## 3. Four-partition model

**ACCEPTED**

```text
EFI    — FAT32
SYSTEM — OS-managed
DATA   — Btrfs
SWAP   — optional
```

EFI and SYSTEM are hidden from ordinary users. DATA is the normal user-visible storage area. SWAP is optional and can be partition/file/ZRAM-based.

**OPEN:** exact SYSTEM filesystem.

## 4. Multi-disk installations

**ACCEPTED direction**
- EFI/SYSTEM and DATA/SWAP do not have to be on the same physical disk.
- Installer should be able to recommend sensible layouts based on disk sizes and retained-image/kernel requirements.
- DATA failure must be recoverable through Recovery mode.

**OPEN:** exact installer layout algorithm and behavior for degraded/missing disks.

## 5. EFI and bootloader

**ACCEPTED**
- Custom `luna-boot.efi`.
- Very small and stable.
- No normal boot menu unless `B` is pressed.
- Boot menu can expose OS selection, Recovery and USB/external boot options.
- EFI is not auto-mounted for the user.
- No GUI is required/assumed for EFI; the earlier discussion did not establish a graphical EFI UI.

## 6. System Images

**ACCEPTED**
- System Image = directly a SquashFS payload.
- System Images live only on SYSTEM.
- Each image has a manifest beside it.
- No global manifest.
- No `.lbp` System Images.
- An image without a valid manifest must not be shown by the bootloader.
- The filename/version can be the primary identity; the manifest provides metadata such as compatible kernels.
- System Images are read-only/immutable during normal operation.

## 7. Factory and fallback

**ACCEPTED**
- `current` = current image/kernel combination.
- `factory` = guaranteed factory-good image/kernel combination.
- Factory is retained.
- Factory recovery needs both a factory System Image and a factory kernel path.
- System Image failure should attempt a compatible previous image without reboot where technically possible.
- Kernel panic is handled after reboot by selecting another compatible kernel.
- Fallback always respects image/kernel compatibility.

## 8. Kernel management

**ACCEPTED**
- Kernels are independent versioned entities on SYSTEM.
- Kernel management is separate from application/bundle management.
- Current kernel must not be deleted.
- Old kernels may be removed by the privileged updater/OS management component.
- User does not directly manage SYSTEM contents.

**OPEN:** exact kernel file/bundle structure, metadata and factory-kernel representation.

## 9. Boot state and retention

**ACCEPTED**
- Do not rewrite boot state on every ordinary boot.
- State changes only on relevant events.
- Retention count is configurable.
- Example discussed: last four normal System Images plus factory.

**OPEN:** exact boot-state format and exact retention algorithm.

## 10. DATA structure — correction history

Earlier documents contained a simpler structure:

```text
data/
├── system/
│   ├── apps/
│   └── drivers/
├── users/
├── data/
└── cache/
```

**SUPERSEDED by later explicit decision:** DATA is now intended to be exposed as:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   └── volumes/
├── users/
└── cache/
```

The user explicitly rejected redundant structures such as `DATA/apps`, `DATA/portable`, `DATA/system/system`, etc.

## 11. User directory correction

**ACCEPTED**

```text
DATA/users/<user>/
├── home/
├── data/
└── config/
```

`home` = ordinary user files.
`data` = application/user mutable data.
`config` = user/application configuration.

This is a per-user structure and avoids conflicts between users sharing the same DATA partition.

## 12. Applications

**ACCEPTED**
- Applications are macOS-like bundles/directories.
- Applications live in `DATA/system/apps` and are shared between users.
- The same application is not copied separately for each user.
- Application bundles are immutable as installed units in normal operation, but the user may deliberately inspect/open a bundle and manually adjust files/metadata for compatibility. This is an explicit advanced-user capability, not the normal installation path.
- Application data/configuration does not live inside the immutable bundle.
- Application data persists after uninstall until explicitly cleaned or an automatic retention policy removes unused data.
- A dedicated Apps view in the file manager is desired; it should show installed applications and launch them by double-click.
- Heavy applications may eventually be stored on other disks/removable media as portable bundles.

**OPEN:** exact bundle structure and portable-bundle registration rules.

## 13. Bundle format

**ACCEPTED direction**
- `.lbp` is the separate Luna Bundle Format.
- It is distinct from System Images.
- RFC-0002 is intended to define Bundle Format v1.

**OPEN:** exact binary/container format, manifest, dependency model, signatures, installation transaction, etc.

## 14. Libraries and dependencies

**ACCEPTED direction**
- Libraries live under `DATA/system/libs`.
- Dependencies should be isolated and addressable, conceptually taking the useful part of Nix-like isolation.
- Applications can receive different versions of a library without a global conflict.

## 15. Drivers

**ACCEPTED**
- Drivers live under `DATA/system/drivers`.
- Drivers are independently managed entities.
- Recovery should be able to disable/remove a broken driver without destroying the OS image.

**OPEN:** exact distinction between immutable kernel modules and mutable/user-installable drivers. The user accepted the conceptual split but the concrete taxonomy remains open.

## 16. Logical Linux root

**ACCEPTED**
- The physical DATA partition must not reproduce the normal Linux root tree.
- The kernel/applications should nevertheless see a conventional logical `/`.
- The logical root is assembled in RAM/virtual filesystem space.
- System Image content can be supplied lazily from SquashFS.
- Only required data should be loaded initially; other blocks can become available on demand.

## 17. `luna-root` and mapping architecture

**ACCEPTED direction**
- `luna-root` is responsible for constructing/managing the logical root mapping layer.
- Existing Linux mechanisms should be reused where practical rather than inventing an entire filesystem/container mechanism.
- Do not put every responsibility into one monolithic init component.

The architecture combines:

```text
minimal RAM root
+ System Image content
+ DATA-backed mappings
+ application/user namespaces
```

## 18. Path mapping rules

**ACCEPTED**
- Mappings are semantic and policy-controlled.
- No unrestricted physical-to-logical path rewriting.
- Configuration paths such as `/etc` may map to user configuration.
- A random user path must not automatically become an executable/system path.
- Mapping tables are per application namespace, not global.
- Layered lookup is conceptually:

```text
application → user → system
```

This is similar in spirit to layered lookup in Python, but it is not Python's import mechanism.

## 19. Namespace isolation

**ACCEPTED**
- Every application receives its own mount/filesystem namespace.
- The namespace contains only required application resources, user resources, system resources and explicitly allowed external volumes.
- Different applications can map `/libs/gtk` to different physical GTK versions.
- Namespace state/mapping tables may remain in RAM after application exit for a managed period. One hour was discussed as an example; exact timeout is open.

## 20. Application permissions

**ACCEPTED direction**
- Applications do not receive unrestricted access to all volumes by default.
- User decides which locations/devices an application can access.
- Permissions should be integrated with namespaces and file-picking/user-mediated access.

**OPEN:** exact permission model, prompts and APIs.

## 21. External volumes

**ACCEPTED UX**
- Windows/macOS-like automatic appearance.
- User sees disk/volume labels, not `/mnt`, `/dev/sdb1`, etc.
- A connected disk should already be usable; the user should not have to open it once before other programs can access files.
- Internal managed state can live under `DATA/system/volumes`.

**OPEN:** exact mount backend and device-manager implementation.

## 22. Configuration precedence

**ACCEPTED**

```text
DATA user configuration
        ↓ if present
System Image default
```

Defaults are immutable inside the image; modifications create/maintain mutable DATA copies.

## 23. Sessions and users

**ACCEPTED in Phase 1.2**
- Multiple users may have simultaneous active sessions.
- Each user can independently be ACTIVE, RESTRICTED or TERMINATED.
- Default when leaving a session: RESTRICTED.
- System services may continue independently across user switches.
- An updater can continue an update transaction while user A leaves and user B becomes active.

## 24. Recovery and DATA failure

**ACCEPTED in Phase 1.2**

```text
minimal RAM root
    ↓
System Image
    ↓
attach DATA
    ↓
normal session
```

If DATA cannot be attached/used → Recovery.
If System Image cannot be started → Factory.

The idea is to avoid attaching DATA first and then discovering that the OS itself failed.

## 25. Btrfs checkpoint/rollback

**ACCEPTED direction**
- Btrfs snapshots are useful for a dedicated checkpoint/rollback subsystem.
- This is not continuous full-DATA snapshotting by default.
- User chooses between previously discussed option 2 and option 3, or disables the feature.
- Default is option 2.
- Snapshots should be visible/manageable by the user.

**OPEN:** exact snapshot scope, naming, retention, automatic checkpoints and rollback transaction semantics.

## 26. Swap/ZRAM

**ACCEPTED direction**
- Swap partition or swap file.
- Optional swap.
- ZRAM supported.
- Installer and settings expose policy.
- ZRAM may be useful even when persistent disk swap is absent.

## 27. Desktop

**ACCEPTED direction**
- niri + Noctalia Shell.
- Ghostty + fish.
- Desktop layer must remain separate from core architecture.

## 28. Service management

**WORKING**
- Prefer an existing Linux service model rather than inventing one from scratch.
- OpenRC-like behavior was the chosen direction.
- Exact service manager integration is still open.

## 29. Component architecture

**ACCEPTED**
Components/crates should have clear responsibility and minimal knowledge of other components.
Existing workspace components that have built successfully:

```text
luna
luna-common
luna-log
luna-fs
luna-bundle
luna-config
```

Do not create empty future components before their implementation work begins.

## 30. Development order

**ACCEPTED**

```text
Architecture
    ↓
RFC
    ↓
Format
    ↓
Interfaces
    ↓
Prototype
    ↓
Implementation
    ↓
Integration
```

Do not write large amounts of code before the architecture/interface is agreed.

## 31. Rust learning requirement

**ACCEPTED**
The user has Python experience and is learning Rust. Future Luna Rust code must be educational:
- explain important types;
- explain ownership/borrowing when relevant;
- explain `struct`/`enum` choices;
- explain `Result`/`Option`;
- explain traits/lifetimes when material;
- explain module/crate boundaries;
- compare to Python when useful;
- avoid clever abstractions when a clearer educational implementation is reasonable.

## 32. Known corrections to preserve

1. Factory image **and factory kernel** are required.
2. DATA root is `system`, `users`, `cache`; not a collection of duplicated `data`/`apps`/`portable` roots.
3. `DATA/users/<user>` contains `home`, `data`, `config`.
4. `DATA/system` contains `apps`, `drivers`, `libs`, `volumes`.
5. System Images and kernels remain on SYSTEM, never silently moved to DATA.
6. EFI/SYSTEM are hidden from the ordinary user.
7. No GUI for EFI was established as a requirement.
8. Namespace mappings are per-application, not global.
9. `/etc` mapping is allowed as a configuration class, but arbitrary DATA paths cannot rewrite arbitrary Linux paths.
10. Hybrid SquashFS loading is intended; exact implementation remains open.

## 33. Important open questions at the end of the current phase set

- Exact SYSTEM filesystem.
- Exact installer auto-partition algorithm.
- Exact factory kernel representation.
- Exact System Image manifest schema.
- Exact kernel metadata schema.
- Exact `.lbp` format.
- Exact `current`/`factory` persistent state format.
- Exact hybrid SquashFS/RAM implementation.
- Exact `luna-root` process/API boundary.
- Exact logical path classes for `/etc`, `/usr`, `/var`, `/lib`, etc.
- Exact per-namespace mapping table format.
- Exact application permission model.
- Exact namespace persistence/timeout.
- Exact device automount backend.
- Exact Btrfs checkpoint scope/retention.
- Exact session manager.
- Exact updater transaction semantics across user switches.
- Exact service manager.

## 34. Rule for future edits

Never silently overwrite an accepted decision.

When a new proposal conflicts with an accepted decision, label it `ARCHITECTURE CONFLICT`, explain the conflict and obtain an explicit decision before updating the Source of Truth.

---

# Phase 1.2 — Decisions M–Q (2026-08-16)

## M — Runtime root construction

**Decision: C — hybrid approach.**

Luna will use a hybrid boot/runtime root model. The exact implementation may use an initramfs-like minimal environment, but the architecture does not commit to a particular Linux implementation yet.

The logical goal is a minimal RAM root followed by lazy/hybrid System Image availability rather than copying the entire System Image into RAM at once.

## N — Mapping granularity

**Decision: option 2 — map individual required files rather than entire directories.**

Logical Linux paths may correspond to files stored in different physical Luna locations. A whole-directory mapping would be too coarse and could introduce unwanted files or conflicts.

Examples include logical `/etc/...` and user configuration paths such as `.../home/.config/app/...`.

This reinforces the Phase 1.1 rule that mappings are explicit, namespace-local, and policy constrained.

## O — Runtime mapping model

The previously proposed mapping behavior is accepted. Each application namespace receives its own small mapping table and layered lookup rather than one global mapping table.

## P — Btrfs snapshots

Btrfs snapshots are a **checkpoint/rollback and recovery mechanism**, not the mechanism for normal runtime session switching.

## Q — Recovery user

**Decision: Recovery mode runs the OS without normal DATA and without the user's persistent local sessions.**

Recovery uses a virtual recovery user whose data is stored in RAM rather than persistent DATA.

Therefore:

```text
DATA unavailable
      ↓
System Image still starts
      ↓
Recovery user in RAM
      ↓
Recovery environment
```

If the System Image itself cannot start, the boot path proceeds toward Factory mode instead.

This is an explicit correction/clarification of any older description that treated Recovery merely as a bootloader state or assumed DATA was required.
