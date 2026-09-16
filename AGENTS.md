# Project Luna — Agent Context

## Authority

This file is the operational context for coding agents working in Project Luna.
Current architecture and contracts are authoritative.
Direct user requests and repository contracts override this file when they conflict.
Do not invent new architecture when an accepted Luna decision already exists.

## Project identity

Project: Project Luna. Internal name: `luna`.
Implementation language: Rust. License: Apache License 2.0.
Linux kernel is the foundation; Luna is its own OS architecture.
Primary target: a real, bootable and usable Alpha on the developer laptop.

## Canonical storage

Physical layout:
EFI | LUNA-SYS | LUNA-DATA | SWAP
LUNA-SYS = ext4. LUNA-DATA = Btrfs.
For Alpha, EFI, LUNA-SYS and LUNA-DATA are on the same physical disk.
`LUNA-SYS/config/luna-data.toml` binds the expected DATA disk and partition GUIDs.

## System image

A normal System Image is directly `luna-X.Y.Z.squashfs`.
Its manifest is `luna-X.Y.Z.toml` beside it in `LUNA-SYS/images/`.
`.lbp` is a Bundle format, never a System Image format.
System Images are immutable and versioned.

## Boot chain

UEFI -> luna-boot.efi -> Linux kernel -> luna-init (PID 1)
-> luna-system-runtime -> UserSession -> luna-app-runtime -> ApplicationInstance
`luna-init` remains PID 1. `luna-system-runtime` is its child.
`luna-boot.efi` resolves System Image -> compatible init -> compatible kernel.

## Architecture boundaries

Keep luna-init as the only PID 1 and preserve the direct handoff execution path.
Do not restore `luna-core`, generic `luna-runtime`, or `luna-session` as mandatory layers.
Do not use `luna-run-session` as an architectural layer.
Do not use user boot profiles or trusted-setup as architecture.
Do not make `/run` a whole-system staging directory.
Do not silently substitute unrelated LUNA-DATA.

## Logical root

Running `/` is assembled from approved immutable System Image resources and mutable DATA.
Do not copy the whole System Image into RAM.
`/run` may exist as a normal runtime filesystem, but it is not a root staging mechanism.
`/dev`, `/proc`, `/sys`, `/run`, and `/tmp` are runtime resources.

## DATA model

LUNA-DATA/system contains apps, drivers, firmware, libs, config, state and volumes.
LUNA-DATA/users/<user> contains home, data and config.
`system/state/` and `system/volumes/` are required.
Drivers and firmware are distinct resource classes.

## Recovery and boot state

Recovery is a normal System Image + compatible init + compatible kernel + virtual DATA.
Recovery DATA comes from `LUNA-SYS/recovery/`; physical DATA is a repair object.
`current`, `fallback`, `factory`, and `recovery` are semantic boot roles.
Boot attempt NVRAM state is minimal and durable; detailed progress is RAM-only.
Normal boot must not rewrite durable boot state without a meaningful state change.

## Runtime and libc

Luna's native userspace stack is musl-native.
`RuntimeSpec` distinguishes `musl` and optional `glibc` compatibility runtime.
Old `RuntimeKind::{Luna,Glibc,Bundle}` semantics are stale.
Bundled applications are a future ecosystem; Bundle installation is not an Alpha blocker.

## Alpha desktop

Baseline: niri, Noctalia Shell, Noctalia Greeter, Ghostty, fish,
Luna Files GUI for Yazi, Yazi, Firefox, and a CLI editor (prefer micro/nano plus nvim).
Alpha hardware priority: Intel UHD 630/i915, Intel AX200 Wi-Fi/Bluetooth,
Realtek r8169 Ethernet, Intel HDA audio, USB xHCI, NVMe and SATA.
NVIDIA proprietary support is not an Alpha blocker.

## Development workflow

Primary development branch is `alpha-development`; keep `develop` protected.
Inspect existing work before modifying files. Never reset or discard unrelated changes.
Prefer small, testable changes and logical Git commits.
Build and test after meaningful changes; use QEMU before physical-disk deployment.
Never claim a feature works without evidence.

## Safety

Autonomous repository, build, test, QEMU and documentation work is allowed.
Never repartition or erase a physical disk without explicit confirmation immediately before it.
Do not expose secrets in files, logs, commits, prompts or reports.

## Required engineering behavior

Prefer the smallest implementation that satisfies the accepted architecture and contract.
Reuse existing Luna components when they already provide the needed boundary.
Treat compilation, tests, QEMU output and hardware tests as evidence, not assumptions.
When implementation and current accepted documentation disagree, reconcile them explicitly.
