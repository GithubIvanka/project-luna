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

Development builds now use the two managed slots in `target/canonical-workbench`.
Resource-only experiments rebuild the small System Image inside the experimental
disk; sequential QEMU cases use disposable snapshots/overlays. Retention,
promotion and cleanup commands: [ARTIFACT-WORKFLOW.md](ARTIFACT-WORKFLOW.md).
The `dist` artifact above remains the explicit export path.

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
The current core applications are luna-device-manager, luna-volume-manager, luna-system-runtime and luna-user-session.
The image also contains the canonical login font, Luna artwork and immutable bootstrap configuration.

## DATA

LUNA-DATA remains the mutable provider boundary.
The current foundation stores Niri and its musl runtime closure under system/apps/niri and system/libs.
The logical-root builder exposes those libraries through /usr/lib and maps the musl ELF loader to /lib/ld-musl-x86_64.so.1.
The persistent Luna user is stored under users/luna/{home,data,config}.
Normal and Recovery DATA include `cache`, `system/state` and `system/volumes`.
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
  -> luna-device-manager + luna-volume-manager (automatic volume availability)
  -> native Luna UserSession
  -> DRM/KMS login UI
  -> luna/luna authentication
  -> /usr/bin/luna-graphical-session
  -> user D-Bus + PipeWire/WirePlumber
  -> Niri + Noctalia
~~~

Fresh 2026-10-10 full-image OVMF tests verify handoff, direct PID 1, DATA mounting, the manager gate before boot success, native login and authenticated Niri/Noctalia. The audited providers restore private Mesa VirGL and ordinary system fonts. The older dated notes below describe their original verification scope.
The canonical source image is tested through snapshot copies, so its checksum remains stable after boot.

## QEMU graphics fixture

The canonical OVMF tests use `-vga none -device virtio-vga-gl` and SDL OpenGL.
This supplies the same Virtio DRM/VirGL path while preserving VGA fallback
during firmware startup. `LUNA_QEMU_GPU=virtio-gpu-gl-pci` reproduces the earlier
firmware surface gap in the display regression. CPU screendump after accelerated
scanout is best effort; the actual desktop was captured with Niri screenshot.

## 2026-10-09 — Device Manager / Volume Manager boot gate

The normal boot path now makes the two Luna storage managers required startup services.
`luna-system-runtime` starts and supervises `luna-device-manager` first; it owns
kernel uevents, performs coldplug, and atomically publishes
`/run/luna/device-manager.volumes`. The runtime then starts
`luna-volume-manager`, which waits for that inventory, applies auto-attach policy,
records `volume_count` and `available_count`, and publishes its readiness marker.
OpenRC does not start either Luna manager; it is invoked only after boot success
for optional external service runlevels.

Before confirming boot success, `luna-system-runtime` verifies that the native
uevent monitor is active, at least one block partition was discovered, auto-attach
is enabled, and at least one Volume is available. Failure leaves the boot attempt
unconfirmed and prevents the login UI from starting. The OVMF regression now
requires the manager-readiness log to precede the boot-success marker.

Verified locally: focused manager/runtime tests, workspace check, static
x86_64-musl builds, and the QEMU manager-gate acceptance pass. A canonical disk
image was built in an isolated output root and its EFI, LUNA-SYS, System Image,
and LUNA-DATA structure was inspected by `tools/test-canonical-luna-disk-image.sh`.
The production OVMF boot of that newly built canonical disk was not part of the
2026-10-09 verification. The 2026-10-10 full-image audit now covers it.

## Transitional Alpine foundation

Physical LUNA-SYS still contains the complete Alpine 3.24.2 foundation because this branch keeps the proven direct-root Alpine control checkpoint.
Production luna-init does not use that Alpine tree as the logical root. It uses the minimal System Image plus the selected DATA provider.
This redundancy stays isolated to alpine-foundation until the direct-root control path is retired.

## Remaining work

The full canonical image has fresh production boot and graphical acceptance;
see [GRAPHICS-BRANCH-AUDIT.md](GRAPHICS-BRANCH-AUDIT.md) for evidence, lifecycle
checks and the visually inspected Niri screenshot. Each common DATA resource
namespace has one mount owner: UDisks policies merge into the dbus-daemon
dbus-1/polkit-1 resources rather than hiding every other provider's policies.

PipeWire process startup does not prove audio output. DNS, physical hardware,
full Alpha application baseline and recovery/installation remain separate work.
The DATA namespace omission has been corrected. Tracked input preparation and
the full regression entrypoint are described in
[ALPHA-STABILIZATION.md](ALPHA-STABILIZATION.md). Retirement of the
transitional Alpine root is a subsequent change, not part of this regression fix.

The experiment remains isolated from develop.
