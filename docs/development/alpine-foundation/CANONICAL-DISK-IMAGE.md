# Canonical Luna disk image — Alpine foundation milestone

## Target

The alpine-foundation experiment builds one GPT disk image with exactly four partitions:

~~~text
Disk
├── EFI
├── LUNA-SYS
├── LUNA-DATA
└── SWAP
~~~

EFI is FAT32, LUNA-SYS is ext4, LUNA-DATA is btrfs, and SWAP is a dedicated swap partition.
The canonical versioned artifact is dist/luna-X.Y.Z.img. dist/luna-pc.img remains a compatibility symlink.

## Boot artifacts

LUNA-SYS now contains the real Luna boot artifact set:

~~~text
LUNA-SYS/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   └── luna-X.Y.Z.toml
├── cores/
│   ├── luna-X.Y.Z.init
│   └── luna-X.Y.Z.toml
├── kernels/<kernel-id>/
│   └── bzImage
├── config/
│   ├── boot-state.toml
│   └── luna-data.toml
└── recovery/
    ├── recovery.squashfs
    └── recovery.toml
~~~

luna-init is a separate LUNA-SYS core artifact. It is not embedded as a System Image application.

## Minimal System Image

The System Image is now a minimal immutable Luna source rather than an Alpine root snapshot.
It contains the accepted namespaces apps, drivers, firmware, libs, config and resources.
The current core applications are luna-device-manager, luna-system-runtime and luna-user-session.
The image also contains the canonical login font, Luna artwork and immutable bootstrap configuration.

## DATA

LUNA-DATA remains the mutable provider boundary.
The current foundation stores Niri and its musl runtime closure under system/apps/niri and system/libs.
The logical-root builder exposes those libraries through /usr/lib and maps the musl ELF loader to /lib/ld-musl-x86_64.so.1.
The persistent Luna user is stored under users/luna/{home,data,config}.
The builder no longer creates a physical /luna-data symlink inside the persistent user home.

## Verified production path

~~~text
UEFI
  -> luna-boot
  -> Luna kernel 7.2.4
  -> memory-resident luna-init (PID 1)
  -> LUNA-SYS + System Image + LUNA-DATA
  -> RAM-backed logical /
  -> luna-system-runtime
  -> luna-device-manager
  -> native Luna UserSession
  -> DRM/KMS login UI
  -> luna/luna authentication
  -> /usr/bin/niri --session
~~~

Fresh OVMF tests verify Luna handoff acceptance, direct PID 1 execution, LUNA-DATA btrfs mounting, logical-root preparation, runtime boot success, DRM/KMS login rendering, keyboard/pointer input, development authentication and Niri launch.
The canonical source image is tested through snapshot copies, so its checksum remains stable after boot.

## QEMU graphics fixture

The production OVMF regression uses -vga none with virtio-gpu-pci. The default QEMU stdvga device is not a supported DRM device in the current Luna kernel configuration.

## Transitional Alpine foundation

Physical LUNA-SYS still contains the complete Alpine 3.24.2 foundation because this branch keeps the proven direct-root Alpine control checkpoint.
Production luna-init does not use that Alpine tree as the logical root. It uses the minimal System Image plus the selected DATA provider.
This redundancy stays isolated to alpine-foundation until the direct-root control path is retired.

## Remaining work

The current canonical DATA image does not yet contain the external system services expected by the existing runtime; D-Bus, NetworkManager, BlueZ and UDisks are currently reported as unavailable.
The next phase is to package and verify those provider responsibilities, then verify the full Niri process lifetime and Wayland session.
Only after that should we consider removing the transitional Alpine root payload from physical LUNA-SYS.

The experiment remains isolated from develop.