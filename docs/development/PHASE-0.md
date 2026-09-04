# Project Luna — Phase 0: Bootable Baseline Contract

Phase 0 is the stabilization gate before expanding Luna with new user-facing features. Its purpose is to make the current architecture reproducible, testable, and bootable end-to-end.

## 1. Goal

Reach a repository state in which the current Luna boot chain is internally consistent and every required automated check is green:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init (standalone musl initramfs)
  ↓
SYSTEM / DATA discovery
  ↓
versioned SquashFS System Image
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
Wayland → niri → Noctalia
```

Phase 0 is a contract and acceptance gate, not a feature milestone.

## 2. Frozen contracts

### Storage

The physical disk model is:

```text
EFI
SYSTEM
DATA
[SWAP]
```

`SYSTEM` is system-managed and contains versioned images and kernels. `DATA` contains mutable state. The development PC image uses ext4 for SYSTEM and DATA.

### System Image

A System Image is a SquashFS filesystem image directly:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
```

`.lbp` is a bundle/archive transport format and is not a System Image.

### Kernel pairing

System Images and kernels are independently versioned, but the bootloader must only select a kernel declared compatible by the image manifest.

`current` means the confirmed working image + kernel combination. `factory` means the retained known-good factory combination and is not removed by normal retention.

### Early userspace

`luna-init` is a standalone early-userspace program, built statically for `x86_64-unknown-linux-musl`. It may obtain SYSTEM/DATA devices from kernel command-line parameters and must validate the selected System Image path before mounting it.

### Runtime

`luna-system-runtime` is the system supervisor. `UserSession` owns the graphical user-session lifecycle. There is no separate `luna-session` or `luna-run-session` architecture component in the final System Image.

## 3. Required repository gates

All of the following must pass before Phase 0 is considered complete:

1. `cargo fmt --all -- --check`
2. workspace tests and the repository's configured Clippy checks
3. standalone `luna-init` build/check for `x86_64-unknown-linux-musl`
4. UEFI `luna-boot.efi` build for `x86_64-unknown-uefi`
5. PC image build using `tools/build-pc-image.sh`
6. documentation consistency checks; no obsolete boot/session architecture claims

Warnings that indicate a real correctness issue are treated as failures rather than hidden by CI configuration.

## 4. Boot acceptance

The development image must provide all artifacts required by the boot chain:

- UEFI loader in the EFI system partition.
- Linux kernel compatible with the selected System Image.
- Standalone `/init` from `luna-init`.
- SYSTEM image and adjacent manifest.
- DATA filesystem with the expected label.
- Final System Image containing `luna-system-runtime` as `/sbin/init` and the configured graphical login/session commands.

A successful smoke boot must reach the graphical login/session path without relying on a TTY login fallback.

## 5. Failure-path acceptance

The boot design must retain deterministic recovery behavior:

```text
selected image/kernel
        ↓ failure
previous compatible pair
        ↓ exhaustion
factory pair
        ↓ failure
recovery
```

The loader must not rewrite boot state on every successful boot. State changes are tied to meaningful boot events such as a newly selected pair, confirmed success, or recorded failure.

## 6. Reproducibility

The development PC image is the reference integration artifact. A rebuild must use the repository's documented toolchain and host dependencies and produce the same logical partition layout and boot contract.

Phase 0 does not require byte-for-byte identical disk images unless deterministic timestamps and file ordering have been explicitly fixed. It does require deterministic contents and interfaces from the repository's perspective.

## 7. Explicit non-goals

Phase 0 does not attempt to complete:

- desktop visual polish;
- application ecosystem or application packaging expansion;
- broad networking/audio/Bluetooth feature work beyond what is required to boot;
- large architectural refactors unrelated to the frozen boot contract;
- production update distribution infrastructure.

Those are subsequent phases and must not weaken the Phase 0 baseline.

## 8. Exit criteria

Phase 0 is complete when:

- required GitHub checks are green on `main`;
- the documented PC build completes;
- the resulting image satisfies the frozen EFI/SYSTEM/DATA layout;
- `luna-boot.efi` can discover a valid System Image and compatible kernel;
- `luna-init` can discover the labeled SYSTEM/DATA devices and mount the selected image;
- `luna-system-runtime` starts the configured graphical login/session path;
- the documentation describes the same architecture as the code;
- no extra development branch is required to represent the canonical repository state.

Only after these conditions are met should new feature work be treated as the next phase.
