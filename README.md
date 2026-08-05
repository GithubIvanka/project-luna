# Project Luna

**Design first. Code second.**

>A modern immutable operating system built on the Linux kernel.

Project Luna is an open-source operating system focused on simplicity, predictability and long-term maintainability.

Rather than extending the traditional Linux distribution model, Project Luna rethinks the operating system architecture while remaining compatible with the Linux kernel.

The project aims to provide a clean system layout, immutable core, self-contained applications and a modern desktop experience built entirely around Wayland.

> **Status:** Early design stage. Nothing is implemented yet.

---

# Vision

Project Luna follows a few simple ideas:

- Immutable core operating system
- Minimal system architecture
- Human-readable filesystem layout
- Self-contained applications
- Atomic system updates
- Rust-first development
- Wayland-only desktop
- Documentation before implementation

---

# Goals

## Simplicity

Every component should have a single responsibility.

## Predictability

The system should always behave the same way.

## Reliability

System updates must be atomic and recoverable.

## Transparency

Configuration should be readable and understandable by humans.

## Performance

Modern software stack without unnecessary legacy components.

---

# Architecture

The operating system is divided into three logical layers.

```
EFI
│
├── Boot
│
System
│
└── Immutable operating system
│
Users
│
├── Applications
├── User data
├── Configuration
└── Cache
```

The system itself is immutable.

Applications and user data are stored separately from the operating system.

---

# Core Technologies

| Component | Technology |
|-----------|------------|
| Kernel | Linux |
| Programming Language | Rust |
| Display Server | Wayland |
| Compositor | niri |
| Shell | fish |
| Terminal | Ghostty |
| Desktop | Noctalia Shell |

---

# Repository Structure

```
docs/
src/
tools/
```

Documentation is written before implementation.

---

# Current Status

The project is currently focused on designing the operating system architecture.

Current work includes:

- Filesystem layout
- Boot process
- Service manager
- Package format
- Application model
- Update mechanism

Implementation will begin after the core architecture is finalized.

---

# License

Project Luna is licensed under the Apache License 2.0.

See the LICENSE file for details.
