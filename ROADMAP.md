# Project Luna — Roadmap

This roadmap describes the development sequence for Project Luna.

> The architectural Source of Truth is [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
> This file reflects the accepted development order, not fixed deadlines.

---

## How to read this file

- **Sequence and dependencies** come from the Source of Truth (sections 85, 98, 104).
- **Time estimates** are *proposals only*. They have not been accepted and must
  not be treated as committed deadlines.
- The project is **design-first**. Per section 98, the priority is:

```
Architecture → RFC → Format → Interfaces → Prototype → Implementation → Integration
```

not "write a lot of code first and reconcile architectures later".

---

## Where the project is now

Per section 108.19 / 109.10:

- Phases 1.1–1.5 are **accepted and consolidated**.
- Phase 1.5 established the contracts between Security, Root Mapping,
  Application Runtime, System Runtime, UserSession, IPC, Events/Operations and
  Diagnostics/Health.
- The **next phase** is repository/crate architecture — translating those
  contracts into concrete Rust workspace boundaries and APIs.

Current code state (section 86): a Rust workspace exists and builds
(`luna`, `luna-common`, `luna-log`, `luna-fs`, `luna-bundle`, `luna-config`).
No bootloader, bundle format, System Image, kernel manager, runtime or device
manager is implemented yet.

---

## Development sequence

The following order is taken from section 85 and section 104. Each stage must
reach sufficient precision before the next one begins (section 98).

### Stage 0 — Repository / crate architecture (current)
Translate Phase 1.5 contracts into concrete Rust workspace boundaries and APIs.

- Define crate responsibility boundaries per section 105.23.
- Keep `luna-common` small (not a dumping ground).
- Create new crates only when a real architectural boundary exists (section 51.1).
- Produce concrete API sketches for the Phase 1.5 contracts.

**Exit criteria:** crate map and API boundaries are precise enough to start
specification and early prototypes without architectural rework.

### Stage 1 — Architecture baseline
Consolidate and confirm the foundation (largely already in the Source of Truth):

- directory layout;
- system/data boundary;
- boot architecture;
- System Image architecture;
- kernel architecture;
- bundle architecture.

**Exit criteria:** the four-layer model (EFI → BOOT → SYSTEM → DATA, section 96)
is unambiguous and stable.

### Stage 2 — RFC-0001
The main architectural RFC describing the Luna foundation:

- purpose;
- principles;
- disk layout;
- boot architecture;
- system/data separation;
- immutable model;
- versioned images;
- kernel separation.

**Exit criteria:** RFC-0001 accepted and consistent with section 105 onward.

### Stage 3 — RFC-0002 (Bundle Format v1)
Design `.lbp` (section 33). Must define:

- bundle structure;
- metadata;
- manifest;
- payload;
- versions and identifiers;
- dependencies;
- architecture / target platform;
- installation, update, removal;
- integrity checks;
- signatures (if accepted);
- compatibility;
- storage rules;
- file format inside the bundle.

**Exit criteria:** `.lbp` format specified enough to implement `luna-bundle`
and `luna-app-manager` import logic.

> Remember (section 32): `.lbp` is the Bundle Format. System Image is SquashFS.
> These are independent formats.

### Stage 4 — System Image Specification
Separate from the bundle (section 34). Must cover:

- SquashFS payload;
- per-image manifest;
- kernel compatibility;
- boot metadata;
- versioning;
- retention.

**Exit criteria:** System Image + manifest + compatibility + retention specified.

### Stage 5 — Kernel Specification
Cover:

- kernel metadata;
- versioning;
- compatibility;
- installation / removal;
- selection;
- fallback;
- kernel verification.

**Exit criteria:** kernel lifecycle and compatibility model specified.

### Stage 6 — Boot Specification
Describe the boot path (section 85, 93):

```
UEFI → luna-boot.efi → current → manifest → compatible kernel
     → System Image → fallback → factory
```

**Exit criteria:** `luna-boot.efi` behaviour, boot state and fallback are
specified without inventing unspecified formats.

### Stage 7 — Prototype
Begin implementation **after** the specifications above are precise enough
(section 85, stage 6 / section 104).

Suggested first prototypes (not yet committed):
- `luna-boot.efi` minimal boot path;
- bundle parse/validate prototype;
- System Image mount/materialization prototype.

### Stage 8 — Implementation
Build out the components against the accepted specifications, one component at
a time, following the single-component rule (section 99).

### Stage 9 — Integration
Integrate components end-to-end and validate against the canonical models
(sections 92–95).

---

## Dependency graph

```
Stage 0 (crate/API architecture)
   │
   ▼
Stage 1 (architecture baseline)
   │
   ▼
Stage 2 (RFC-0001)
   │
   ├──────────────────────┐
   ▼                      ▼
Stage 3 (RFC-0002)    Stage 4 (System Image Spec)
   │                      │
   │                      ▼
   │                 Stage 5 (Kernel Spec)
   │                      │
   └──────────┬───────────┘
              ▼
        Stage 6 (Boot Spec)
              │
              ▼
        Stage 7 (Prototype)
              │
              ▼
        Stage 8 (Implementation)
              │
              ▼
        Stage 9 (Integration)
```

Notes:
- RFC-0001 (foundation) should precede format-specific RFCs.
- System Image and Kernel specs inform the Boot spec.
- Prototype should not begin before the relevant format/spec is precise enough.

---

## What must NOT be treated as done

Per section 68, the following are **not yet defined** and must be designed,
not assumed, when their stage is reached:

- exact `.lbp` binary format / structure;
- exact Bundle and System Image TOML manifests;
- exact `current` / `factory` formats;
- exact kernel metadata structure;
- exact kernel-panic detection and soft-fallback procedures;
- exact SquashFS / hybrid loading implementation;
- exact application permission model;
- exact device automount backend;
- exact OpenRC/service integration;
- final `luna` CLI;
- exact runtime namespace structure;
- bundle/image signatures and cryptographic verification policy;
- update transaction protocol.

---

## Proposed (non-committed) time framing

> ⚠️ These are illustrative proposals only. No dates or durations have been
> accepted in the Source of Truth. Replace with real estimates once the team
> explicitly accepts them.

| Stage | Suggested effort | Status |
|-------|------------------|--------|
| Stage 0 — crate/API architecture | TBD (proposal) | 🔜 next |
| Stage 1 — architecture baseline | TBD (proposal) | largely done in SoT |
| Stage 2 — RFC-0001 | TBD (proposal) | 📋 not started |
| Stage 3 — RFC-0002 | TBD (proposal) | 📋 not started |
| Stage 4 — System Image Spec | TBD (proposal) | 📋 not started |
| Stage 5 — Kernel Spec | TBD (proposal) | 📋 not started |
| Stage 6 — Boot Spec | TBD (proposal) | 📋 not started |
| Stage 7 — Prototype | TBD (proposal) | 📋 not started |

---

## Guiding constraints for every stage

From the Source of Truth:

1. **Design first** — do not start large amounts of code before the
   architecture is precise (section 98).
2. **One component at a time** — responsibility, boundaries, inputs, outputs,
   dependencies, state, API, errors, then code, then tests (section 99).
3. **No silent architectural changes** — accepted decisions are not changed
   silently; conflicts are marked `ARCHITECTURE CONFLICT` (sections 88, 105.28).
4. **Don't confuse "discussed" with "decided"** (section 103).
5. **Respect the critical prohibitions** (section 72) — e.g. never move System
   Images into DATA, never make System Image a `.lbp`, never use one global
   manifest, never force boot-state rewrites on every boot.

---

## Maintenance

When a stage completes:
- consolidate its accepted decisions into `docs/ARCHITECTURE.md`;
- update this roadmap's statuses;
- keep phase documents as historical/traceability material only (section 105.29).
```
