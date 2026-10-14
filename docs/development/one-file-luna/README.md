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

## Stage 1.5 result — 2026-10-07

The ambiguous-DATA safety path was validated. The fixture contains two valid GPT partitions named `LUNA-DATA`, with distinct partition GUIDs and no configured GUID pair selecting either one.

`luna-boot.efi` reports DATA as unavailable for normal selection because the provider is ambiguous and automatically switches to the Recovery target. The same real `luna-init` then mounts the Recovery DATA Image, materializes its `recovery-tools` provider into the logical namespace, and reaches `LUNA-SYSTEM-RUNTIME: READY` without mounting either ambiguous physical DATA partition.

This closes both normal DATA safety branches currently exercised by the one-file experiment: missing DATA and ambiguous DATA both fail closed into Recovery rather than silently selecting an arbitrary provider.

## Stage 1.6 result — 2026-10-07

The positive configured-DATA selection path was validated. The fixture contains two valid `LUNA-DATA` partitions, but `LUNA-SYS/config/luna-data.toml` contains an exact disk GUID + partition GUID pair for the first DATA target.

`luna-boot.efi` resolves the configured pair before generic candidate discovery and selects the configured partition even though another valid `LUNA-DATA` is present. The real `luna-init` receives `mode=0`, mounts `/dev/sda3`, prepares the logical root and starts `luna-system-runtime`.

Verified path:

UEFI/OVMF -> configured GUID pair -> selected physical LUNA-DATA `/dev/sda3` -> real `luna-init` PID 1 -> logical root -> `luna-system-runtime`

The one-file experiment now exercises all three DATA selection outcomes: an exact configured match is accepted, no valid provider is treated as missing and enters Recovery, and multiple unconfigured providers are treated as ambiguous and enter Recovery.

## Stage 1.7 result — 2026-10-07

The cross-disk DATA binding path was validated. The fixture places EFI and `LUNA-SYS` on the first virtual disk and places the only physical `LUNA-DATA` partition on a second virtual disk. `LUNA-SYS/config/luna-data.toml` stores the second disk GUID and DATA partition GUID.

`luna-boot.efi` discovers the configured DATA provider across all available block devices rather than restricting discovery to the System disk. The handoff carries the selected partition GUID to the kernel, and the real `luna-init` resolves that GUID independently through `/sys/block` and mounts the external provider as `/dev/sdb1`.

Verified path:

UEFI/OVMF -> Disk A EFI + LUNA-SYS -> configured external Disk B LUNA-DATA -> real `luna-init` PID 1 -> Btrfs `/dev/sdb1` -> logical root -> `luna-system-runtime`

This proves the intended physical separation invariant: EFI and LUNA-SYS remain bound to one disk, while LUNA-DATA may live on another physical disk without introducing a second boot stack or changing the direct PID 1 mechanism.

## Stage 1.8 result — 2026-10-07

A stale configured-DATA GUID pair was tested as a fail-closed regression. The fixture contains one valid physical `LUNA-DATA` partition, while `LUNA-SYS/config/luna-data.toml` points to a nonexistent partition GUID.

Before the fix, the bootloader correctly failed the exact GUID lookup but then fell through to generic candidate discovery and silently selected the only remaining `LUNA-DATA` partition. The QEMU trace showed normal boot (`mode=0`) and `/dev/sda3`, confirming a real policy violation.

The implementation in `components/boot/src/filesystem.rs` was changed so that once an explicit disk+partition GUID pair is configured, failure to resolve that exact pair returns `DataStatus::Missing` immediately. Generic discovery is no longer allowed to substitute another provider.

After the fix, the same stale-config fixture enters Recovery, mounts the Recovery DATA Image, and reaches `LUNA-SYSTEM-RUNTIME: READY`. The positive exact-match test was rerun afterward and still selects the configured `/dev/sda3` normally.

This establishes the required fail-closed invariant for persisted DATA identity: configured DATA is either the exact configured provider or Recovery; it is never silently replaced by an arbitrary candidate.

## Stage 1.9 result — 2026-10-07

The logical-root provider contract was promoted from a mount-only check to an end-to-end runtime assertion. The test runtime now verifies the actual namespace assembled by `luna-init` after the `chroot` transition.

The acceptance checks cover all seven System Image resource classes under `/resources`, all seven DATA resource classes under `/usr/share`, System Image configuration under `/config`, provider configuration under `/etc` and `/data/system/config`, System Image libraries under `/lib/luna`, DATA libraries under both `/usr/lib` and `/usr/local/lib`, a DATA application materialized in `/usr/bin`, the DATA musl loader mapping, and the selected user's persistent home mapping.

The complete provider matrix was then exercised:

```text
normal configured DATA       -> logical-root contract -> READY
exact configured DATA        -> logical-root contract -> READY
external-disk DATA           -> logical-root contract -> READY
missing DATA                 -> Recovery DATA -> logical-root contract -> READY
ambiguous DATA               -> Recovery DATA -> logical-root contract -> READY
stale configured DATA        -> Recovery DATA -> logical-root contract -> READY
```

This closes the current boot/provider experiment: the same real `luna-init` and System Image contract work across physical DATA on the System disk, physical DATA on another disk, and VirtualData supplied by Recovery, while stale or ambiguous persistent identity fails closed.

## Stage 4.0 — Luna-owned device/volume startup gate — 2026-10-09

The normal direct-init path now starts the real `luna-device-manager` and
`luna-volume-manager` as supervised child processes of `luna-system-runtime`.
OpenRC is not required to start either Luna component and no longer owns their
default-runlevel entries. The runtime verifies the native uevent monitor, a
non-empty block inventory, enabled volume auto-attach, and at least one available
Volume before confirming boot success. Early manager exit or failed readiness
keeps boot success unconfirmed.

The experiment exposed two minimum kernel/userspace contracts:

- The one-file `tinyconfig` kernel must enable `CONFIG_NET=y` so
  `NETLINK_KOBJECT_UEVENT` is available to the native Device Manager.
- When no udev database exists, Device Manager performs read-only ext4/Btrfs
  superblock signature checks. This is deliberately limited to the filesystems
  needed for `LUNA-SYS` and `LUNA-DATA`; it is not a general replacement for all
  external filesystem metadata or compatibility providers.

Verified OVMF path:

```text
UEFI/OVMF
  -> luna-boot.efi
  -> Linux 7.2.4 direct-init kernel (CONFIG_NET=y)
  -> real luna-init as PID 1
  -> System Image + Btrfs LUNA-DATA -> logical root
  -> luna-system-runtime
  -> luna-device-manager (native uevent; 3 block partitions discovered)
  -> luna-volume-manager (LUNA-DATA available, alias C:)
  -> manager readiness gate (detected=3, available=1, auto_attach=true)
  -> boot success confirmed
```

The acceptance trace confirms both managers become ready before
`luna-system-runtime: boot success confirmed`. The manager gate itself passes in
the headless OVMF fixture. The same run does **not** yet verify a working graphical
login: after the boot gate, the headless environment reports `session UI I/O failed:
No such file or directory`. That is a separate UserSession/graphics acceptance
problem and must not be conflated with Device/Volume Manager readiness.

The reproducible direct-init kernel builder now enables `CONFIG_NET` so this
contract is preserved when the experiment kernel is rebuilt.

## Stage 4.1 — manager-gate regression tests — 2026-10-09

The real OVMF acceptance harness is now repeatable through
`experiments/luna-onefile/test-manager-boot.sh`:

```sh
bash experiments/luna-onefile/test-manager-boot.sh all
```

The suite has three cases:

- **Positive:** the real Device Manager and Volume Manager run from the System
  Image. Their readiness and the runtime's gate log must all precede
  `luna-system-runtime: boot success confirmed`.
- **Missing Volume Manager:** the manager executable is deliberately omitted
  from the fixture. Runtime startup must fail and boot success must remain
  unconfirmed.
- **No available volumes:** a small static test provider reports
  `auto_attach=true`, `volume_count=1`, and `available_count=0`. This
  deterministic fault injection exercises the real runtime's readiness
  validator without damaging or falsifying the Btrfs DATA fixture. Boot
  success must remain unconfirmed.

All three individual OVMF cases passed on 2026-10-09. In the two negative
cases, the serial log contains the expected runtime startup rejection and no
`boot success confirmed` marker. The current `luna-init` fatal-child path
then terminates PID 1 and the kernel reports a panic. That is fail-closed, but
it is not a graceful recovery transition; early runtime failure handling
remains a separate design task.

The dedicated harness now uses the existing test helpers directly, so there
are no generated temporary copies or edits to the user's test scripts required
to run the manager-gate checks.

## Stage 4.2 — native login surface in headless OVMF — 2026-10-09

The headless login failure was traced to the minimal experimental kernel rather
than to the runtime gate: the direct-init tinyconfig lacked DRM and evdev.
`SeatController` enumerates `/dev/dri`, and UserSession's input backend
discovers `/dev/input/event*`; without `CONFIG_DRM` and
`CONFIG_INPUT_EVDEV`, seat discovery returned `ENOENT` before KMS
initialization.

The reproducible direct-init kernel builder now enables and verifies
`CONFIG_DRM=y`, `CONFIG_DRM_BOCHS=y`, and
`CONFIG_INPUT_EVDEV=y`. The OVMF fixture explicitly uses QEMU `-vga std`,
which matches the Bochs DRM driver.

The positive OVMF serial trace now verifies this ordered path:

```text
luna-system-runtime: boot success confirmed
luna-user-session: login UI graphics device /dev/dri/card0
luna-user-session: login UI acquired DRM master
luna-user-session: login UI KMS target connector=39 crtc=37 mode=1280x800@75Hz
luna-user-session: login UI initial frame presented
luna-user-session: login UI first frame drawn
luna-user-session: login UI keyboard/pointer input ready
```

The manager-gate regression suite was rerun against this kernel. The positive
case must reach first frame and input readiness; negative manager cases still
must not produce the boot-success marker.

This proves the native login UI can acquire a KMS surface, draw its first frame,
and initialize keyboard/pointer input in the test fixture. It does not yet
prove interactive authentication: the harness does not inject keystrokes or
verify that a user's desktop session starts. The fixture has no bundled font,
so the renderer's documented bitmap fallback remains expected and non-fatal.

## Stage 4.3 — interactive native login and unprivileged session handoff — 2026-10-09

The login UI acceptance was extended from drawing a frame to a real keyboard
submission and a normal user-session process launch. The direct-init kernel now
also enables and verifies `CONFIG_MULTIUSER=y`. This is required: Linux's
`tinyconfig` with MULTIUSER disabled compiles out UID/GID credential syscalls,
while Luna correctly calls `initgroups`, `setresgid`, and `setresuid` before
launching the graphical session.

The dedicated test fixture enables a temporary SHA-512-crypt password only for
the `luna` test account; Recovery is marked `nologin` so the ordinary login
screen has one eligible account. The QEMU HMP keyboard injector sends actual
key events into the native UI. It does not bypass SessionUi or authentication.

Verified path in QEMU/OVMF:

```text
manager readiness -> boot success
-> DRM master + KMS frame + keyboard input ready
-> keyboard-entered test password
-> luna-user-session: login credentials accepted
-> user credential syscalls succeed with CONFIG_MULTIUSER=y
-> luna-graphical-session launched as the authenticated user
-> LUNA-NATIVE-SESSION: READY
```

The session fixture checks that the bootstrap runtime exists at
`/sbin/luna-system-runtime`, its actual logical-root location, and verifies
the configured Niri file and DATA-provided resources expected by a normal user
session. Recovery-only home paths are not prerequisites for this normal-mode
acceptance.

The acceptance command is:

```sh
bash experiments/luna-onefile/test-manager-boot.sh all
```

The four scenarios cover positive manager/KMS readiness, missing Volume
Manager, zero available volumes, and interactive authentication plus native
session launch. The test password is synthetic and exists only in the disposable
test image; it is not written into project configuration or host accounts.

All four scenarios passed as one complete suite on 2026-10-09 using the
Linux 7.2.4 direct-init kernel with `CONFIG_NET`, `CONFIG_DRM`,
`CONFIG_DRM_BOCHS`, `CONFIG_INPUT_EVDEV`, and `CONFIG_MULTIUSER` enabled.
In the interactive case the serial trace contains both
`login credentials accepted` and `LUNA-NATIVE-SESSION: READY`, with no kernel
panic after the authenticated user process is launched.

## Stage 4.4 — Niri Wayland compositor through Luna UserSession — 2026-10-09

The interactive direct-init fixture now launches the real Niri compositor
from the DATA provider after the native Luna login succeeds. The guest uses
Luna's repository-built Mesa VirGL artifact from
dist/.build/alpine-foundation-graphics/mesa/out, not Alpine's generic Mesa
package, because the latter does not supply the required VirGL Gallium driver.
The QEMU acceptance case uses virtio-gpu-gl-pci with egl-headless; the kernel
reports +virgl and both capability sets.

This run also identified three minimal-kernel contracts that were missing from
tinyconfig:

- CONFIG_BINFMT_SCRIPT=y to execute the user-session launcher via its
  #!/bin/sh interpreter.
- CONFIG_FILE_LOCKING=y because Wayland creates its socket with a flock
  lock file. Without file locking, Niri's automatic socket selection returns
  AlreadyInUse despite an empty runtime directory.
- CONFIG_INOTIFY_USER=y so the DATA-provided eudev/udevd compatibility
  service can initialize inotify instead of failing the Device Manager gate.

luna-init now assigns ownership of persistent per-user home, data, and
config directories using UID/GID values from the selected auth provider.
This lets the authenticated user create its fontconfig cache and normal home
files. luna-system-runtime starts only the optional seatd provider as a
transitional libseat compatibility layer for the Niri test. It does not
replace Luna-native UserSession seat ownership. The Device Manager remains
the single owner of udevd startup, coldplug and supervision.

The verified OVMF trace reaches:

    luna-system-runtime: boot success confirmed
    luna-user-session: login credentials accepted
    luna-system-runtime: graphical UserSession launched /usr/bin/luna-graphical-session
    Niri: Wayland socket /run/user/1000/wayland-1
    Niri: render node /dev/dri/renderD128
    Niri: new connector Virtual-1
    Niri: connecting connector Virtual-1
    Niri: mode 1280x800 selected

The compositor remains running through the test's 60-second QEMU window. The
trace no longer contains "virtio_gpu: driver missing", "software EGL renderers
are skipped", or "no allocator available for device". Mesa still logs a warning
that EGL_WL_bind_wayland_display is unsupported; the compositor continues to
initialize the DRM output. A pixel screenshot was attempted through QEMU's
HMP screendump, but the egl-headless backend reports "Error: no surface".
Therefore the test proves Wayland, renderer and output setup, not a visually
inspected framebuffer or a window-containing desktop frame.

### Host acceptance rerun — 2026-10-10

The current `luna-init`, `luna-system-runtime`, Device Manager, Volume Manager,
and direct-init kernel were rebuilt from the working tree before rerunning the
Niri acceptance. This Codex host has no `/dev/dri`, and its QEMU 10.2 GTK backend
was built without OpenGL support, so the default `egl-headless` display could
not start VirGL. The harness now accepts `LUNA_NIRI_QEMU_DISPLAY`; using
`sdl,gl=on` supplied the required host GL surface while keeping the guest's
virtio-gpu VirGL path unchanged.

The full five-case manager-gate suite passed with the rebuilt artifacts:

```text
positive manager gate
missing Volume Manager blocks boot success
zero available volumes blocks boot success
interactive login launches the native session
Niri session creates its output and remains running for 60 seconds
```

The Niri case passed the complete 60-second session acceptance:

```text
manager gate and boot success
-> DRM master, login frame, and keyboard input ready
-> keyboard-entered test password accepted
-> seatd started as the optional libseat compatibility provider
-> Wayland socket /run/user/1000/wayland-1
-> Niri connected Virtual-1 at 1280x800
-> compositor remained alive through the test timeout
```

Run it on a host that needs the SDL backend with a fresh output directory:

```sh
test_out="$(mktemp -d /tmp/luna-niri-session.XXXXXX)"
LUNA_MANAGER_GATE_TEST_OUT="$test_out" \
LUNA_NIRI_QEMU_DISPLAY='sdl,gl=on' \
bash experiments/luna-onefile/test-manager-boot.sh niri-session
```

This verifies compositor startup and continued operation through QEMU's SDL
surface. The acceptance still checks serial/DRM state rather than captured and
visually inspected pixels.

The reproducible direct-init kernel builder now enables and verifies
CONFIG_NET, CONFIG_DRM_VIRTIO_GPU, CONFIG_BINFMT_SCRIPT,
CONFIG_FILE_LOCKING, CONFIG_INOTIFY_USER, and the earlier DRM, evdev and
multi-user settings. The Niri provider adapter overlays only the existing
Luna-built VirGL Mesa files into an isolated DATA tree; it does not change the
system-installed Mesa or build the full Alpine desktop.

## Stage 4.5 — Noctalia shell through Luna UserSession — 2026-10-10

The tracked Alpine graphics build recipe now produces a complete Noctalia
runtime instead of relying on an untracked package-cache layout. It stages the
Noctalia executable and assets, resolves APK virtual package providers, and
includes a D-Bus session configuration for the test user bus. The canonical
disk builder now copies Noctalia's assets into DATA alongside the executable;
the runtime maps them to `/usr/share/noctalia` and the wrapper exports
`NOCTALIA_ASSETS_DIR`.

The separate `noctalia-session` OVMF case keeps the existing Niri acceptance
and native Luna login path, then starts Noctalia with `spawn-at-startup`. Its
serial assertions require an authenticated user session, a live Wayland
socket, the D-Bus session bus, Wayland layer-shell, EGL 1.5, the Mesa VirGL
renderer, the default wallpaper and top bar, and a Noctalia process that stays
alive until QEMU ends the observation window. The staged DATA provider is
about 367 MiB, so this case gets a 1 GiB LUNA-DATA partition; the existing
Niri-only test retains its 768 MiB partition.

The test passed on 2026-10-10 with Alpine 3.24.2, Noctalia 5.2.0, and the
repository-built Mesa 26.1.6 VirGL stack. The guest log reports a 1280x800
`Virtual-1` output, EGL 1.5, `virgl (Mesa Intel(R) UHD Graphics 630 (CFL GT2))`,
the default wallpaper and bar, and a working session bus. This is serial-log
evidence of Noctalia initialization and shell surface creation; no guest pixel
capture was available for visual inspection. PipeWire/audio and plugin
installation are outside this test case.

On this host the build needed the cached Alpine image and host networking:

```sh
LUNA_ALPINE_GRAPHICS_BUILD_ROOT=/tmp/luna-alpine-graphics-build \
LUNA_ALPINE_IMAGE=alpine:3.24.2 \
LUNA_DOCKER_NETWORK=host \
bash tools/build-alpine-foundation-graphics.sh

LUNA_ALPINE_GRAPHICS_BUILD_ROOT=/tmp/luna-alpine-graphics-build \
LUNA_MANAGER_GATE_TEST_OUT="$(mktemp -d /tmp/luna-noctalia-session.XXXXXX)" \
LUNA_NIRI_QEMU_DISPLAY='sdl,gl=on' \
bash experiments/luna-onefile/test-manager-boot.sh noctalia-session
```

The test-only provider and its cached binaries stay under the selected build
root. This acceptance does not make Alpine, Niri, Noctalia, or the D-Bus
provider Luna-owned architecture layers.

## Stage 4.6 — full canonical desktop and lifecycle audit — 2026-10-10

The full GPT canonical disk was compared with the working Alpine direct-root
launcher. Existing user changes and the untracked fixture provider were kept.
The audit found private Mesa being overwritten by service libraries, missing
ordinary fonts, overlapping D-Bus/Polkit resource mounts, incomplete service
accounts/FHS aliases, and desktop processes not registered with UserSession.
These are repaired in the existing Luna boot/init/runtime/provider boundaries.

The full-image tests now cover native login, a 60-second Niri/Noctalia session,
logout followed by a second login, actual guest poweroff and reboot, and QEMU
firmware display continuity. The display default is `virtio-vga-gl`; the old
`virtio-gpu-gl-pci` intentionally fails the new firmware display check. Native
boot shares the existing Alpine GOP framebuffer handoff helper.

Full analysis, exact evidence and current limits are recorded in
[GRAPHICS-BRANCH-AUDIT.md](../alpine-foundation/GRAPHICS-BRANCH-AUDIT.md).
The ordinary image and separate action-injecting test images are under
`target/luna-foundation-audit-20261010/`. This full-image acceptance is distinct
from the earlier one-file Niri/Noctalia fixtures; those fixtures and user work
were preserved. No commit or physical deployment was performed.
