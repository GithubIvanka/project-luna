# Project Luna

**Design first. Code second.**

> A modern immutable operating system built on the Linux kernel.

Project Luna is an operating system built around the Linux kernel. It is **not** a conventional Linux distribution. Instead of shipping a modified package set on top of a standard userspace, Luna defines its own architecture for the system image, boot process, applications, filesystem layout and system management — while reusing Linux kernel mechanisms where they are useful.

> **Status:** Early design stage. Architecture is consolidated through Phase 1.5. Core specifications (RFC-0001, RFC-0002) are in progress. Implementation begins after the architecture is precise enough.

---

## The core idea

Project Luna is inspired by the **One File Linux** concept:

> The system should have a very small, stable and maximally immutable foundation.

Everything else is built on top of that foundation in a controlled, versioned way.

Luna is built around four layers:

```
EFI      → launches
BOOT     → decides what to launch
SYSTEM   → versioned OS images and kernels
DATA     → everything mutable
```

---

## What Luna is NOT

Luna is deliberately **not**:

- a regular Linux distribution with a different package set and theme;
- a Docker-like operating system;
- a system with a large number of top-level root directories;
- a system where every application scatters files across `/`;
- a system where updating the kernel requires rewriting the whole OS;
- a system where updating the OS destroys user data;
- a system where the user must manually mount USB drives via the terminal;
- a system where the bootloader rewrites boot state on every launch;
- a system where a System Image is a `.lbp` bundle.

---

## Architecture overview

For the full architecture, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — the single Source of Truth.

### Disk layout

Luna uses four disk areas:

```
Disk
├── EFI       — bootloader storage (luna-boot.efi)
├── SYSTEM    — versioned System Images (SquashFS) + kernels
├── DATA      — mutable user-visible storage (Btrfs)
└── SWAP      — swap policy (optional: partition, file and/or ZRAM)
```

### System Images

A **System Image** is a version of Luna itself. It is stored **directly as a SquashFS image** — not as a bundle:

```
SYSTEM/images/
├── luna-1.0.0.squashfs
├── luna-1.0.0.toml        ← per-image manifest
├── luna-2.0.0.squashfs
├── luna-2.0.0.toml
└── ...
```

Each System Image has its **own manifest** next to it. Kernels are stored and versioned independently in `SYSTEM/kernels/`. System Images and kernels can be updated and rolled back **independently**.

### Boot

```
UEFI → luna-boot.efi → compatible kernel → System Image → logical root → DATA
```

- `luna-boot.efi` is Luna's own minimal UEFI bootloader.
- Press **B** during startup to open the Boot Menu.
- Boot supports **soft fallback**: if a System Image fails, Luna tries a previous compatible image without rebooting. A kernel panic triggers a reboot and selection of a previous compatible kernel.
- `current` identifies the active image/kernel combination.
- `factory` identifies the original guaranteed-good installation and is never deleted by normal cleanup.

### Applications

Applications are **immutable bundles** using the `.lbp` format, installed under `DATA/system/apps/`. Each application runs in its own filesystem/mount namespace and sees a clean logical root, not the host filesystem.

```
DATA/system/
├── apps/       — installed application bundles
├── drivers/    — drivers
├── libs/       — shared libraries
├── volumes/    — managed external volumes
└── config/     — system-wide mutable configuration
```

### Users

Every local user has exactly three top-level directories:

```
DATA/users/<user>/
├── home/       — Documents, Downloads, ...
├── data/       — mutable application data
└── config/     — user/application configuration
```

### System management

The system is managed through a single tool, `luna`, which is a thin client over backend services. There is **no permanent root user** and no `sudo`/`su` privilege hierarchy — administrative authority is a protected, per-operation capability.

---

## Core technologies

| Component | Technology |
|-----------|------------|
| Kernel | Linux |
| Language | Rust |
| Build system | Cargo |
| System Image format | SquashFS |
| Bundle format | `.lbp` |
| Configuration | TOML |
| Display protocol | Wayland |
| Compositor | niri |
| Desktop shell | Noctalia Shell |
| Terminal | Ghostty |
| Shell | fish |
| License | Apache-2.0 |

---

## Repository structure

```
project-luna/
├── components/          # Rust workspace crates
│   ├── luna/           # main CLI entry point
│   ├── luna-bundle/    # bundle format representation
│   ├── luna-common/    # shared fundamental types
│   ├── luna-config/    # configuration
│   ├── luna-fs/        # filesystem abstraction
│   └── luna-log/       # logging
├── docs/               # documentation
│   ├── ARCHITECTURE.md # Source of Truth
│   └── rfc/            # RFC documents
├── Cargo.toml          # workspace manifest
├── CHARTER.md          # project charter
├── HISTORY.md          # project history
├── STATUS.md           # current project status
├── ROADMAP.md          # development roadmap
└── README.md           # this file
```

> Architectural subsystems appear in the repository **only when their real development starts**. Empty crates for future subsystems are not created in advance.

---

## Development philosophy

1. **Architecture first** — Architecture → RFC → Format → Interfaces → Prototype → Implementation. Not "code first, figure it out later".
2. **Modular crates** — every crate must answer "why does this crate exist?". `luna-common` is not a dumping ground.
3. **Immutable where possible** — System Images and application bundles are immutable.
4. **Event-driven state** — boot state changes only on events, not on every boot.
5. **Compatibility-aware boot** — only manifest-declared compatible kernels are considered.
6. **No silent changes** — accepted decisions are never changed silently. Conflicts are marked `ARCHITECTURE CONFLICT`.
7. **Don't confuse "discussed" with "decided"** — a new idea is a `Proposal` until explicitly `Accepted`.

---

## Getting started

Project Luna is currently in the design phase. The Rust workspace builds successfully but contains foundational scaffolding rather than a working OS.

### Prerequisites

- Rust toolchain (see `rust-toolchain.toml`)
- Git

### Building

```bash
cargo build
```

---

## Documentation

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — the single architectural Source of Truth
- **[STATUS.md](STATUS.md)** — current project status
- **[ROADMAP.md](ROADMAP.md)** — development roadmap
- **[CHARTER.md](CHARTER.md)** — project charter
- **[HISTORY.md](HISTORY.md)** — project history

---

## License

Project Luna is licensed under the **Apache License 2.0**. See [LICENSE](LICENSE).
```
