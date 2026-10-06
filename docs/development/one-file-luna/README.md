# Luna One File Experiment

**Stage:** 0 — One File Luna
**Status:** Active experiment
**Branch:** `luna-onefile-experiment`
**Baseline:** cleaned `alpine-foundation`, commit `e69b5ef`

## Purpose

This is a clean-room experimental path for Luna OS development. The experiment starts from the smallest practical Linux boot environment and grows Luna outward from the kernel entry point.

It intentionally does not treat Alpine Linux, Void Linux, LFS, systemd, OpenRC, runit, or the current Luna desktop stack as architectural foundations.

## Stage 0 target

```text
UEFI
  ↓
Linux kernel
  ↓
embedded initramfs
  ↓
luna-init (PID 1)
  ↓
Luna core proof
  ↓
LUNA-READY
```

The complete boot artifact is one EFI-loadable kernel image containing the Stage 0 initramfs.

## Stage sequence

1. **Stage 0 — One File Luna**: prove a one-file EFI image can reach a Luna-owned PID 1 and deterministic `LUNA-READY` state.
2. **Stage 1 — Luna bootloader**: introduce the Luna-owned EFI bootloader and the boot contract around the kernel.
3. **Stage 2 — LUNA-SYS**: move normal boot content from embedded storage to the canonical `EFI / LUNA-SYS / LUNA-DATA / SWAP` disk model.
4. **Stage 3 — `luna-init` ownership**: make PID 1 own early discovery, validation, image materialization and transition into the Luna runtime.
5. **Stage 4 — Luna system runtime**: replace distribution service supervision with Luna lifecycle ownership.
6. **Stage 5 — Component migration**: move subsystems behind Luna contracts while Linux mechanisms are only temporary implementations/adapters.
7. **Stage 6 — Linux mechanism replacement**: retire Linux userspace mechanisms from the critical path where Luna has a native replacement.

## Rules

- Stage 0 is deliberately tiny.
- No systemd, OpenRC or runit.
- No GRUB/rEFInd dependency.
- No desktop, login manager, DBus, NetworkManager, PipeWire or Bluetooth services.
- `luna-init` is the intended PID 1.
- BusyBox is allowed only as bootstrap material for Stage 0 and must not become a Luna architectural dependency.
- Linux kernel facilities are treated as substrate; Stage 0 does not claim to replace the kernel.
- Each later replacement must have a working earlier stage to compare against.

## External references

**OneFileLinux** supplies the core experimental idea: an EFI-loadable Linux kernel with an embedded userspace image.

**Alpine Linux** is useful as bootstrap material because of its compact musl/BusyBox userspace.

**Void Linux** is a reference for simple service supervision and a deliberately small system structure.

**Linux From Scratch** is a reference for build order and understanding how the Linux userspace is assembled.

These are references only. They are not Luna's source of truth.

## Stage 0 implementation

The implementation lives under `experiments/luna-onefile/` and is intentionally independent of the main Cargo workspace.

The first prototype contains:

- a static musl `luna-init` executable;
- BusyBox only for the bootstrap shell used by the proof;
- a tiny initramfs;
- a Linux EFI-stub kernel image with that initramfs embedded.

## First acceptance test

A QEMU serial boot must show:

```text
LUNA-INIT: PID 1
LUNA-READY
```

and then remain alive without starting any distribution service manager.

## Stage 0 results — 2026-10-06

The first boot experiments established the following:

1. Linux kernel + embedded initramfs reaches `Run /init as init process`.
2. A minimal raw-syscall C `_start` reaches `LUNA-C-READY`, proving the kernel/initramfs execution path works.
3. Rust `std` as PID 1 caused a general-protection fault in this environment, so it is deliberately not part of the Stage 0 critical path.
4. The working Luna PID 1 is now a Rust `no_std` static library entered by a minimal C `_start`, with no libc or Rust `std` runtime before `luna_main()`.
5. The accepted Stage 0 artifact is a Linux EFI-stub `bzImage` with the Luna initramfs embedded. Direct QEMU boot is the current automated acceptance path.
6. OVMF was able to discover and start the EFI-stub image, but this environment did not complete kernel handoff reliably; that UEFI-specific issue remains a follow-up and is not being conflated with the successful direct kernel boot.

The failure mode was useful: it established a hard boundary between the Linux kernel substrate and the userspace runtime we control. Future stages should keep that boundary explicit.
