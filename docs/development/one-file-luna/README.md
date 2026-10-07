[Reading 137 lines from start (total: 137 lines, 0 remaining)]

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

- a static Rust no_std luna-init entered through a minimal C _start;
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
6. The same image was successfully booted through OVMF with the existing luna-boot.efi; the complete path reaches LUNA-READY.

The failure mode was useful: it established a hard boundary between the Linux kernel substrate and the userspace runtime we control. Future stages should keep that boundary explicit.

## Stage 0.1 results — 2026-10-06

The kernel baseline was reduced from upstream x86_64_defconfig to x86_64 tinyconfig plus the explicit Luna Stage 0.1 requirements.

The reproducible builder now:

1. builds the static no_std Rust PID 1;
2. builds a minimal kernel from tinyconfig;
3. applies the Luna kernel requirements with scripts/config;
4. embeds the initramfs directly into the kernel;
5. publishes one EFI-stub bzImage artifact.

Observed artifact characteristics:

- luna-onefile.efi: approximately 1.3 MB;
- static PID 1: approximately 9 KB;
- kernel configuration: approximately 660 defined symbols after Kconfig resolution.

Acceptance is implemented in experiments/luna-onefile/test-stage0.sh.

Direct QEMU and OVMF-through-luna-boot are both accepted paths. The OVMF path is wrapped by experiments/luna-onefile/test-uefi.sh.

## Immediate next work

Stage 1 now moves from proof to hardening: reduce the bootloader dependency surface, make the one-file kernel artifact a first-class boot target, and establish the minimal canonical LUNA-SYS fixture without reintroducing a distribution boot stack.
## Stage 1.0 result — 2026-10-06

The existing Luna EFI bootloader was tested against the one-file kernel artifact.

Verified path:

UEFI/OVMF -> luna-boot.efi -> LUNA-SYS -> one-file Linux bzImage -> embedded initramfs -> Rust no_std luna-init -> LUNA-READY

The test uses the existing OVMF fixture harness and replaces only its kernel input with experiments/luna-onefile/out/luna-onefile.efi.

Acceptance is now wrapped by experiments/luna-onefile/test-uefi.sh.

This establishes that the current luna-boot kernel handoff can consume the experimental one-file kernel without changing the Linux boot contract. The next Stage 1 work is therefore bootloader integration/hardening rather than proving EFI execution from scratch.

[executed on device: pandora (0de7d85d-b652-4002-8307-c060ee52463b)]
## Stage 1.1 result — 2026-10-06

The OVMF test was reduced to a clean canonical disk fixture with no Alpine userspace. The fixture contains only EFI, LUNA-SYS and a single LUNA-DATA partition. LUNA-SYS contains the one-file bzImage, a minimal luna-init record/manifest, and a minimal normal System Image manifest plus empty squashfs payload.

Verified path:

UEFI/OVMF -> luna-boot.efi -> GPT LUNA-SYS/LUNA-DATA discovery -> target resolution -> one-file bzImage -> embedded initramfs -> Rust no_std luna-init -> LUNA-READY

This proves that the bootloader GPT/ext4 discovery, target resolution and Linux handoff do not require Alpine. It does not yet remove the embedded initramfs from the kernel; that is intentionally deferred to the next step, where the Luna kernel consumes the bootloader-provided init identity.

Acceptance is wrapped by experiments/luna-onefile/test-luna-sys.sh.
## Stage 1.2 result — 2026-10-06

The kernel-side direct-init path was verified without an embedded initramfs.

The tested kernel is built from Linux 7.2.4 with the Luna kernel overlay, CONFIG_RUST=y and CONFIG_RELOCATABLE=y. CONFIG_INITRAMFS_SOURCE is absent. The existing luna-boot reads the external luna-init from LUNA-SYS, creates the Linux handoff, and the kernel executes that memory-resident init as PID 1 through luna_exec.

Observed serial path:

luna_boot: boot availability handoff=1 init=1 -> luna_exec: preparing memory-resident luna-init -> luna_exec: executing memory-resident luna-init as PID 1 -> luna_exec: luna-init exec transition accepted -> LUNA-INIT: PID 1 -> LUNA-READY

The Alpine-free canonical GPT fixture is unchanged. The one-file kernel itself no longer supplies PID 1 through initramfs; the init object comes from LUNA-SYS and is transferred through the Luna handoff ABI.

Acceptance is exposed by experiments/luna-onefile/test-luna-direct.sh. The reproducible builder is experiments/luna-onefile/build-direct.sh.

This is the first complete proof of the intended ownership boundary: UEFI/luna-boot owns image discovery and handoff, the Luna kernel owns direct-init execution, and luna-init starts without a pathname-based rootfs pivot.

## Stage 1.3 result — 2026-10-06

The direct-init path was then tested with the real `components/core/luna-init` binary instead of the minimal experiment PID 1.

The acceptance fixture now supplies a minimal but structurally valid System Image and LUNA-DATA filesystem. The System Image contains the runtime, `libs/`, `config/`, and canonical resource directories; LUNA-DATA is Btrfs and is large enough for the filesystem metadata created by the fixture.

Verified path:

UEFI/OVMF -> luna-boot.efi -> GPT LUNA-SYS/LUNA-DATA discovery -> Linux 7.2.4 direct-init kernel -> memory-resident real `luna-init` as PID 1 -> SquashFS System Image mount -> Btrfs LUNA-DATA mount -> logical root preparation -> `luna-system-runtime` -> `LUNA-SYSTEM-RUNTIME: READY`

The first real-init run exposed two useful fixture/kernel dependencies rather than Luna architecture failures: SquashFS support was initially hidden behind `CONFIG_MISC_FILESYSTEMS`, and the Btrfs fixture was too small for the metadata generated by `mkfs.btrfs --rootdir`. Both were corrected and the final acceptance run passed without `Kernel panic`, `general protection`, or pathname `/init` fallback.

Acceptance is exposed by `experiments/luna-onefile/test-real-init.sh`; the reproducible kernel builder is `experiments/luna-onefile/build-real-init.sh`.

This closes the experiment's critical boundary: the real Luna PID 1 can be delivered from LUNA-SYS directly by the Luna handoff ABI and can establish the Luna logical root without an embedded initramfs or a second init process. The next work should focus on validating the full System Image/LUNA-DATA contract rather than changing the direct PID 1 mechanism.

## Stage 1.4 result — 2026-10-07

The missing-DATA recovery path was validated with the same real `luna-init` and direct-init kernel.

For the acceptance fixture, the physical `LUNA-DATA` partition is intentionally absent. `luna-boot.efi` detects that DATA is unavailable and automatically switches the normal boot selection to the Recovery target. Recovery keeps the ordinary System Image + `luna-init` + kernel and adds the singleton Recovery DATA Image.

Verified path:

UEFI/OVMF -> `luna-boot.efi` -> DATA unavailable -> Recovery target -> direct real `luna-init` as PID 1 -> Recovery DATA Image mounted into VirtualData -> logical root -> `luna-system-runtime`

The runtime emitted `LUNA-RECOVERY-DATA: READY`, confirming that Recovery DATA content was materialized into the logical runtime namespace. The normal real-init acceptance was rerun afterward and remained green.

This proves the intended split of responsibility: `luna-boot.efi` owns DATA availability policy and target fallback, while `luna-init` owns the selected physical or VirtualData provider materialization. No separate Recovery System Image and no initramfs are required.
