# Luna Boot — AI Working Rules

This file is persistent working memory for development of `boot/luna-boot`.

## How to work

1. Work directly against the repository `GithubIvanka/project-luna`.
2. The active development branch for the full Luna bootloader work is `feat/luna-boot-full-loader`.
3. Do not ask the user to paste repository files when the required files can be read from GitHub.
4. Inspect the repository first, make justified fixes directly in the development branch, and report what changed.
5. Do not wait for confirmation before making a clearly justified code fix. The user explicitly authorized direct fixes to avoid unnecessary confirmation loops.
6. After a fix, give the user the commit/result and the next concrete test to perform.
7. If a question is genuinely blocking a safe implementation, ask it immediately rather than saying that a question will be asked later.
8. Keep the dialogue focused on implementation, testing, failures, and the next bootloader milestone.

## Current bootloader debugging state

- The user is testing `luna-boot` under QEMU + OVMF on Ubuntu 26.
- OVMF firmware files available on the host include:
  - `/usr/share/OVMF/OVMF_CODE_4M.fd`
  - `/usr/share/OVMF/OVMF_VARS_4M.fd`
- The test disk currently has an EFI partition and an ext4 `system` partition.
- The `system` partition currently contains only `boot/bzImage`; there is not yet a real Luna System Image in the test filesystem.
- The immediate milestone is to get the loader to load and hand off to a Linux kernel first. Proper System Image handling comes afterward.
- The Linux test kernel has been verified as a valid x86_64 relocatable `bzImage`.

## Architecture invariants

- Luna System Image is a SquashFS filesystem image, not a `.lbp` and not a bundle/container format.
- System Image naming is `luna-X.Y.Z.squashfs`.
- Do not use `luna-X.Y.Z.img` for System Images.
- Do not treat the System Image as a container containing SquashFS; the System Image itself is the SquashFS filesystem image.
- `luna-boot` is responsible for boot preparation and Linux handoff; mounting the System Image belongs to early userspace.
- The long-term target model is System Image + compatible Linux kernel + optional initramfs.

## Current observed failure

The loader previously reported:

`Luna boot failed: selected boot target not found`

This was traced to the test filesystem not containing the configured target paths. The immediate development direction was changed to kernel-only boot testing.

After rebuilding and testing, the loader then reached:

`Luna boot failed: UEFI error: ACCESS_DENIED`

The failure occurs before filesystem/kernel preparation completes and is consistent with attempting to open firmware-owned disk/device protocols exclusively.

## Latest fix

`boot/luna-boot/src/block.rs` was changed to open `LoadedImage`, `DevicePath`, and `BlockIO` through `open_protocol(..., OpenProtocolAttributes::GetProtocol)` instead of exclusive access.

Latest commit:

`b53dd65e7381fc77fd3e43e0002e48f6477a3e3d`

Rationale: firmware disk protocols are commonly already opened by UEFI drivers; requesting exclusive access can return `ACCESS_DENIED`. Read-only discovery/access should use non-exclusive `GetProtocol` where the lifetime/concurrency assumptions are controlled.

## Next test

1. User rebuilds `luna-boot` from `feat/luna-boot-full-loader`.
2. User copies the new EFI binary into the QEMU ESP.
3. User boots without pressing `B`.
4. Record the exact first Luna error, if any.
5. If the loader gets past storage discovery, continue to the next failing stage rather than redesigning the architecture prematurely.

## Important debugging principle

Do not solve multiple future layers at once. Advance one boot stage at a time:

UEFI image -> disk discovery -> GPT system partition -> ext4 -> kernel file -> Linux bzImage preparation -> boot params/initrd -> page tables -> ExitBootServices -> Linux entry.

The current goal is the earliest successful Linux handoff, not the final System Image architecture.
