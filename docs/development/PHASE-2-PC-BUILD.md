# Phase 2 — PC Build Handoff

This file is the current continuation note for the `story2` development line.

## Current implemented chain

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux x86_64 kernel
 ↓
small early initramfs
 ↓
SYSTEM
 └── versioned SquashFS System Image
 ↓
DATA
 └── durable Luna state
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
/usr/bin/luna-session
```

The system userspace runtime is built with musl. Runtime choice for applications
is typed independently as `luna`, `glibc` or `bundle`, and is already connected
to mapping and Security contracts.

## What the PC build does

`tools/build-pc-image.sh` creates an x86_64 UEFI/GPT image with EFI, SYSTEM and
DATA partitions, packages `luna-boot.efi`, a versioned SquashFS System Image,
initramfs and a Linux kernel, and creates a persistent DATA filesystem.

The builder never writes to a physical disk. `tools/install-pc-image.sh` is the
separate guarded destructive installation action.

## Current user experience

The default development target is quiet and boots a normal PC display console.
The system starts a `UserSession` and `/usr/bin/luna-session`. Until the final
graphical payload is packaged, the session falls back to `/bin/sh`, keeping the
build useful for development and recovery.

When a prepared desktop root supplies `niri-session` and `/etc/luna/mode`
contains `graphical`, the existing graphical UserSession boundary is used.
The final Wayland/niri/Noctalia integration still requires its own runtime
payload, seat and device/portal wiring.

## Invariants to keep

- SYSTEM is immutable/versioned; DATA is mutable.
- System Image is directly SquashFS.
- `.lbp` remains Bundle transport/archive format.
- `luna-system-runtime` is the sole process supervisor.
- `luna-security` remains the policy authority.
- `luna-root-mapping` remains the mapping layer.
- `luna-namespace` remains the Linux namespace/materialization layer.
- `UserSession` remains the combined user/session entity.
- System Image and kernel remain independently updateable.
- TTY/serial is not the final normal desktop entry path.

## Next large passes

1. Validate the PC image on QEMU and a real UEFI machine.
2. Finish runtime materialization for glibc/Bundle-private runtimes.
3. Add fine-grained device authorization and filtered `/dev`.
4. Integrate the real graphical niri + Noctalia payload.
5. Implement Bundle installation and end-to-end ApplicationInstance launch.
6. Replace development `pre_exec` namespace setup with a production-safe child-creation primitive.

This continuation note supplements `docs/ARCHITECTURE.md` and dated decision records; it does not override accepted architecture decisions.
