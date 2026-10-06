# Alpine Foundation — Session Continuity

Updated: 2026-10-02
Branch: `alpine-foundation`
Workspace: `/home/sibitti/source/alpine-luna`

## Current objective

Use Alpine Linux 3.24.2 x86_64/musl as a known-good userspace while progressively replacing Alpine boot responsibilities with Luna components.

Immediate validation path:

`UEFI -> luna-boot -> Luna kernel -> luna-init -> canonical LUNA-SYS/LUNA-DATA -> luna-system-runtime -> OpenRC -> native Luna login -> Niri/Noctalia`

Direct-root remains a separate control experiment and is not the canonical runtime path. Do not move this design to `develop` automatically.

## 2026-10-02 canonical Luna graphical foundation — verified

The current canonical disk-image experiment now reaches a complete graphical user-session lifecycle without the direct-root Alpine launcher.

Verified artifact:
- `dist/luna-0.1.0.img`
- GPT remains `EFI / LUNA-SYS / LUNA-DATA / SWAP`.
- LUNA-SYS remains ext4; LUNA-DATA remains Btrfs.
- Canonical LUNA-DATA top-level schema is `cach / system / users`; legacy `state`, `volumes`, and `system/state` / `system/volumes` are absent.
- Structural verification: `Canonical Luna disk verification: PASS`.

Verified boot/session path:
`UEFI -> luna-boot -> Luna kernel 7.2.4 -> luna-init PID 1 -> System Image + physical LUNA-DATA -> luna-system-runtime -> OpenRC -> native Luna login -> luna-graphical-session -> user D-Bus -> PipeWire/WirePlumber -> Niri -> Noctalia`.

Verified OpenRC lifecycle:
- OpenRC 0.63.2 starts the sysinit, boot, and default runlevels.
- `seatd`, system D-Bus, elogind, BlueZ, NetworkManager, Polkit, rtkit, and UPower all reach OpenRC `[ ok ]` startup.
- `luna-system-runtime: boot success confirmed` is emitted after the runlevel sequence completes.
- OpenRC owns the system daemon lifecycle; the native Luna UserSession remains the single login/session entry point.

Verified graphical login:
- Native login renderer loads the Luna font and `docs/design/visual-concepts/luna.png`.
- DRM master and QEMU KMS target are acquired at 1280x800@75Hz.
- `luna/luna` authentication succeeds.
- `luna-user-session` launches `/usr/bin/luna-graphical-session`.

Verified desktop session:
- The session launcher creates a real per-user D-Bus session; `dbus-send --session` now reports the bus as ready.
- PipeWire and WirePlumber start inside the user session.
- Niri creates the Wayland display.
- Noctalia is observed as a running process from the Niri startup configuration.
- Niri receives `/dev/input/event0`, `event1`, and `event2` through libseat.
- Production serial confirms libinput creates actual devices: `New device "event1"` and `New device "event2"`. This closes the previous input-discovery blocker.

Regression results:
- `tools/test-canonical-luna-disk-image.sh`: PASS.
- `tools/test-canonical-luna-login-ovmf.sh`: PASS.
- `tools/test-canonical-luna-production-ovmf.sh`: PASS.
- Production image checksum verification: PASS.

Current scope boundary:
- This milestone intentionally keeps Alpine/OpenRC, eudev, libseat/seatd, Niri, Noctalia, PipeWire, WirePlumber and other external providers in place so the working graphical path can be stabilized before replacement work.
- The old direct system-service spawning function remains in source as an unused transitional path and should be removed during the next cleanup phase rather than being used at runtime.
- The next phase is hardening and dependency cleanup: remove redundant transitional plumbing, make provider closures more reproducible, then replace selected external responsibilities with Luna-owned implementations one component at a time.

## Verified

- luna-boot boots Alpine without GRUB.
- Luna kernel 7.2.4 boots Alpine.
- `luna_direct_root=1` mounts the selected EXT4 root directly on `/`.
- Direct-root reaches `/sbin/init` and OpenRC with no actual initramfs/switch_root/chroot/pivot_root.
- EFI GOP stride handoff is fixed: 1280x800x32 now reports Linux `lfb_linelength=5120`.
- Alpine login works after fixing test-root account preparation.
- Test credentials: `luna/luna` and `root/root`.

## Alpine login finding

Stock Alpine initramfs contains `/etc/passwd` and `/etc/group`, but not `/etc/shadow`. Normal authentication occurs after the real rootfs is active.

Reference BusyBox is built with `CONFIG_FEATURE_SHADOWPASSWDS=y`; PAM is disabled. BusyBox login reads shadow credentials from the real rootfs.

Our failed login was caused by our rootfs shadow preparation, not by missing initramfs functionality. The helper now creates proper 9-field shadow records, generates SHA-512 hashes through Alpine BusyBox `mkpasswd`, and validates them.

Important Alpine responsibilities:
- `mkinitfs/initramfs-init.in`: console/getty setup and boot transition.
- `alpine-conf/setup-alpine.in`: root password setup, then `setup-user`.
- `alpine-conf/setup-user.in`: `adduser`, password setup, groups.
- BusyBox `login.c` / `libbb/correct_password.c`: actual authentication.
## Niri state

Native Alpine `niri-25.11-r2.apk` is in `dist/.build/alpine-niri-apk-cache/`. Cache: 85 APKs, about 105 MB.

Niri runtime needs musl, cairo, libdisplay-info, GBM, libgcc, GLib/GObject, libinput, Pango/PangoCairo, PipeWire, pixman, libseat, libudev and xkbcommon.

Kernel prerequisites already present:
- `CONFIG_DRM=y`
- `CONFIG_DRM_VIRTIO_GPU=y`
- `CONFIG_INPUT_EVDEV=y`

`niri-session` falls back to `niri --session` when systemd/dinit are absent.

The direct-root launcher accepts `LUNA_ALPINE_NIRI=1`. It extracts the cached native runtime, adds Alpine seatd OpenRC files, creates the `seat` group, prepares `/run/user/1000`, exports `XDG_RUNTIME_DIR`, and starts QEMU with `-vga none -device virtio-gpu-pci`.

Offline apk installation was tested. A local index resolves the cached set, but package hooks on this host fail with `unshare: Operation not permitted`. Therefore direct APK extraction is currently the reliable image-construction method; do not claim package hooks ran.

The cache also contains `eudev-libs`, while the base root already has `libudev-zero`. APK solving exposed these as conflicting implementations. Do not replace libudev-zero unless the niri test proves it necessary.

## Current blocker / next action

One niri-enabled image build once failed during LUNA-SYS `mkfs.ext4` with an apparent `libLLVM.so.22.1` allocation message. Manual reproduction using the same `system-root` now succeeds, so rerun after cleaning the output before changing architecture.

After successful boot:
1. login `luna/luna`;
2. check `/dev/dri`;
3. verify seatd and `seat` group access;
4. verify `/run/user/1000`;
5. run `niri --version`;
6. run `niri --session` or `niri-session`;
7. record exact DRM/seat/Wayland errors.

Do not add a display manager, GNOME portals, a full desktop stack, or the old glibc niri build yet.
## Files of record

Direct-root launcher:
`components/boot/tests/ovmf/run-alpine-luna-kernel-direct-root.sh`

Login helper:
`components/boot/tests/ovmf/configure-alpine-test-user.sh`

Direct-root kernel overlay:
`tools/apply-luna-direct-root-overlay.py`

Alpine reference tree:
`dist/sources/reference/alpine-3.24/`

Niri cache:
`dist/.build/alpine-niri-apk-cache/`

Direct-root output:
`components/boot/tests/ovmf/out-alpine-luna-kernel-direct-root/`

Successful serial log:
`components/boot/tests/ovmf/out-alpine-luna-kernel-direct-root/serial.log`

## Continuity rule

At each meaningful milestone update this file with verified results, exact current state, blocker, next concrete action, important paths/commands, and Alpine-to-Luna responsibility discoveries.

A new chat should read this file before modifying the Alpine experiment.

## 2026-09-24 status update

- Login path remains verified: `luna/luna` reaches a shell; `root/root` is also prepared for debug access.
- Native Alpine niri runtime is now materially integrated into the test root when `LUNA_ALPINE_NIRI=1`: `niri`, `niri-session`, `seatd`, cached runtime libraries, seat group, runtime directory setup, and a minimal niri config are present.
- Added `wayland-libs-server-1.25.0-r0.apk` to the project cache after the first niri test exposed that the compositor needs the Wayland server library at runtime.
- Added Luna kernel `modules.builtin` metadata to the rootfs when available; this avoids false module-probing failures for built-in DRM devices. The earlier `modprobe: can't change directory to 7.2.4` messages are no longer present in the latest boot log.
- QEMU niri mode now uses `virtio-gpu-gl-pci` with `gtk,gl=on`. This gets the DRM-capable guest GPU configured, but the host QEMU/GTK path currently emits repeated `Gdk-WARNING: eglMakeCurrent failed`. This is a host display-backend issue to resolve before treating guest rendering failures as niri failures.
- The latest GUI boot reaches the normal Alpine login prompt; QEMU is currently not meant to be left running between sessions.
- Previous manual niri launch reached niri itself and proved the binary/runtime entry point works (`niri 25.11`), but the first temporary test config had invalid KDL and the compositor then reported `NoWaylandLib`. A corrected minimal KDL config is now generated by the launcher, and the next test should use the fresh image rather than the old hand-edited config.

### Immediate next action

Run the fresh niri-enabled image with a display configuration that is known to work on this host. Prefer first `virtio-gpu-pci` + GTK without GL to separate DRM/KMS/Wayland from host EGL issues; if niri requires accelerated EGL, test `virtio-gpu-gl-pci` with `gtk,gl=on` separately. Do not add portals or a display manager yet.

## 2026-09-24 graphics milestone — verified

The previous niri rendering blocker is resolved in the direct-root test environment.

Verified end-to-end in a fresh `LUNA_ALPINE_NIRI=1` QEMU image:
- UEFI -> `luna-boot` -> Luna kernel 7.2.4 -> Alpine 3.24 direct root -> OpenRC.
- `seatd` starts and the `luna` user reaches the Alpine shell.
- Native Alpine `niri-25.11-r2` starts on `/dev/dri/card0` / `/dev/dri/renderD128`, selects QEMU `Virtual-1` at 1280x800, and creates a Wayland socket.
- Alpine 3.24 Mesa `mesa-dri-gallium` lacks the virgl Gallium driver needed by the QEMU test. A separate Mesa 26.1.6 build with `gallium-drivers=virgl,llvmpipe` is therefore kept isolated under `/opt/luna-test/mesa-virgl`.
- Niri and Noctalia now use `LD_LIBRARY_PATH`, `LIBGL_DRIVERS_PATH`, and `GBM_BACKENDS_PATH` pointing at that test-only Mesa runtime.
- Noctalia 5.1.0 was built natively against musl on Alpine 3.24 with Meson/C++ and is installed at `/usr/local/bin/noctalia` with its full assets tree.
- The Noctalia ELF closure resolves cleanly inside the Alpine rootfs; `noctalia --version` returns `noctalia v5.1.0`.
- Manual compositor startup successfully exercised Niri `spawn-at-startup`: a Noctalia process appeared as the user and created its IPC sockets, default bar, wallpaper, keyboard/pointer bindings, and EGL context.
- Noctalia renderer is verified as Mesa `virgl` on the QEMU guest: `OpenGL ES 3.2 Mesa 26.1.6`, renderer `virgl (Mesa Intel(R) UHD Graphics 630 (CFL GT2))`.
- The successful Noctalia log shows a working Wayland compositor connection, `layer-shell`, one `1280x800` output, EGL 1.5, and the default top bar/wallpaper.

Known non-blocking warnings in this minimal test environment:
- system D-Bus is not available unless the new test session wrapper starts a bus; Noctalia can still render without it, but power/portal/system integrations remain disabled.
- PipeWire/WirePlumber are now packaged into the test root for the session wrapper; their functionality must be rechecked after the next fresh auto-session run.
- Xwayland-satellite is intentionally absent for this stage.
- The host/QEMU test still reports the expected unrelated ACPI/WMI and missing optional logind properties.

Current direct-root graphical test plumbing:
- `components/boot/tests/ovmf/run-alpine-luna-kernel-direct-root.sh`
- `dist/.build/mesa-virgl-build/direct/`
- `dist/.build/noctalia-build/`
- `dist/.build/noctalia-runtime-apks/`

Immediate next action:

Run one fresh auto-session image after the latest hook change and verify `/var/log/luna-niri.log`, `/var/log/luna-pipewire.log`, and `/var/log/luna-noctalia.log`. If all three are populated, the Alpine graphical foundation can move from manual-session validation to reproducible session startup. Do not move this design to `develop` yet.

## 2026-09-24 auto-session validation — verified

The direct-root graphical test is now reproducibly auto-starting the full user session without manually launching Niri.

Verified from guest ttyS1 after a fresh `LUNA_ALPINE_NIRI=1` image build:
- `dbus-run-session` remains alive with its `dbus-daemon` child.
- `niri --session` is alive as `luna` and listens on Wayland `wayland-1` / Niri IPC.
- `pipewire` and `wireplumber` remain alive under the same user session.
- Noctalia is launched by Niri `spawn-at-startup "noctalia"`; its log reports Wayland compositor/layer-shell binding, EGL 1.5, Mesa virgl renderer, default wallpaper, default top bar, and both IPC sockets.
- WirePlumber reaches `mixer-api ready` after adding the missing `lua5.4-libs` runtime dependency.
- Noctalia now connects to the user session bus and registers `org.freedesktop.Notifications` successfully.

Remaining environment limitations are explicit and non-fatal for the graphical foundation:
- There is no system D-Bus socket (`/run/dbus/system_bus_socket`) yet, so RTKit, logind watchers, and some system-wide integrations remain unavailable.
- WirePlumber therefore reports optional system-dbus/logind and Bluetooth SPA components as unavailable; core PipeWire mixer functionality still reaches `mixer-api ready`.
- The minimal image has no Xwayland-satellite and no CPU temperature sensor.
- Noctalia network-backed palette/template/FX-rate services cannot resolve external DNS inside this test environment; cached/default UI operation is unaffected.

The direct-root graphical path is therefore reproducible through boot, user-session creation, compositor startup, shell startup, and PipeWire/WirePlumber startup. The remaining system-dbus/logind/Bluetooth limitations are service-integration gaps, not blockers for the graphical compositor/shell path.

Next implementation step: decide whether the Alpine foundation experiment should add a real system D-Bus service and Bluetooth SPA support, or keep those as later external-system responsibilities. Do not promote the direct-root experiment to `develop` yet.

## 2026-09-25 graphical greeter + logged-in session — verified

The Alpine direct-root graphical test now reaches a complete graphical login and remains in the logged-in Luna session.

Verified with a fresh QEMU boot using `greetd` + Cage + GTKGreet:
- GTKGreet renders correctly after adding the Alpine GTK/gdk-pixbuf/glycin runtime closure, `gdk-pixbuf` loader cache generation, fontconfig cache generation, and `font-dejavu`.
- GTKGreet is run without `--layer-shell` because Cage does not provide the layer-shell path required by `gtkgreet -l` in this test.
- The greeter runtime uses `/run/greetd-runtime`; its diagnostics/logs no longer write to root-only `/var/log`.
- `/run/seatd.sock` remains `root:seat` with mode `0660`. The synthetic `greetd` user uses primary GID `seat`, while `luna` is also a member of `seat`.
- A graphical login as `luna` was verified. `greetd` starts the user session and does not return to the greeter.
- After login, `niri --session`, `pipewire`, `wireplumber`, and `noctalia` remain alive under user `luna`.
- Niri reports Wayland socket `wayland-1` and its IPC socket; Noctalia creates its Wayland IPC and dmenu IPC sockets and renders the default top bar/wallpaper.
- WirePlumber reaches `mixer-api ready` and `default-nodes-api ready`.

The login-loop root cause was an incorrect userspace redirection: the user session attempted to open `/var/log/luna-{pipewire,wireplumber,niri,noctalia}.log` as `luna`, causing the session command to fail. Session logs are now placed under `$XDG_RUNTIME_DIR`.

The seat-access root cause was also corrected: the greeter and logged-in user both use the `seat` group for the shared `seatd` socket.

Known non-blocking warnings remain: Xwayland-satellite is absent, system D-Bus/logind integration is absent, the QEMU virgl renderer is slow, and Niri reports no default Xcursor icon in the minimal image.

## 2026-09-25 desktop foundation polish — verified

The Alpine direct-root graphical foundation is now reproducible end-to-end with the current Luna kernel 7.2.4 and Alpine 3.24 userspace.

Verified system integration:
- OpenRC cgroups, dbus, elogind, bluetooth, networkmanager, polkit, rtkit, greetd, and seatd all start successfully.
- D-Bus system activation works for org.freedesktop.UPower and org.freedesktop.Accounts; Alpine SystemdService= entries are stripped in the OpenRC test image and dbus-daemon-launch-helper privileges are restored at runtime.
- WirePlumber BlueZ SPA starts without the previous UPower warning after the UPower readiness barrier was added before PipeWire/WirePlumber.
- The QEMU virtio NIC is brought up through BusyBox udhcpc because the experiment intentionally keeps libudev-zero; the guest receives DHCP and DNS works through the QEMU user-network gateway.
- The Alpine mdev init script is replaced in the test image with a modern-kernel-safe coldplug path; the obsolete /proc/sys/kernel/hotplug write is gone.
- The obsolete Alpine kernel.unprivileged_bpf_disabled sysctl is removed from the disposable test image because the current Luna kernel does not expose that sysctl node.

Verified graphical integration:
- Native musl Niri starts automatically and keeps the Mesa 26.1.6 virgl renderer isolated under /opt/luna-test/mesa-virgl.
- Noctalia 5.1.0 starts directly from the Niri config.
- Adwaita icon theme and hicolor fallback are present.
- Adwaita cursor theme is installed, a default cursor inheritance theme is provided, and Niri explicitly sets xcursor-theme Adwaita / size 24.
- gnome-keyring Secret Service is started through the logged-in PAM session.
- loginctl reports the Luna graphical session as active on seat0.

QEMU-only profile:
- Noctalia's system monitor is disabled in 00-luna-qemu.toml because this virtual machine has no physical CPU thermal sensor. This is a test-environment setting only; it does not redefine Luna desktop architecture.

Known non-blocking Niri notices retained intentionally:
1. EGL_WL_bind_wayland_display is unavailable on the current virgl/EGL path, while Niri continues to initialize the compositor and render normally.
2. org.freedesktop.locale1 is absent because this foundation uses OpenRC rather than systemd-localed; the test configuration supplies an explicit XKB layout (us), so keyboard initialization does not depend on locale1.

The current Alpine foundation stage is therefore functionally complete for the direct-root desktop experiment. Do not promote this test launcher to develop yet. The next implementation phase should first make the Mesa/Noctalia recipes and runtime closure reproducible from tracked project tooling rather than from dist/.build state.


## 2026-09-25 desktop foundation final polish — verified

A fresh QEMU boot now verifies the complete Alpine direct-root desktop foundation with KVM, four vCPUs, virtio GPU, QEMU user networking, system D-Bus, elogind, UPower, BlueZ, NetworkManager, polkit, rtkit, greetd, Cage, GTKGreet, Niri, PipeWire, WirePlumber, Noctalia, Xwayland-satellite, icons, cursors, fonts, and GNOME Keyring.

Verified:
- Graphical login remains stable after `luna` authentication and the session does not return to greeter.
- `loginctl` reports the active `luna` session on `seat0/tty1`.
- UPower starts as an OpenRC service before the user session; the earlier WirePlumber/UPower startup race is gone and the WirePlumber warning is no longer present.
- `org.freedesktop.UPower` and `org.freedesktop.Accounts` activate successfully on the real system D-Bus.
- `eth0` receives a QEMU DHCP lease (`10.0.2.15/24`) with a default route through `10.0.2.2`; DNS resolution works. The test uses BusyBox `udhcpc` because this experiment intentionally keeps `libudev-zero` and does not introduce a second udev stack.
- `Adwaita` and `hicolor` icon themes are installed, an Adwaita `default` cursor theme is present, and Niri explicitly selects Adwaita cursor theme/size 24. The greeter also uses Adwaita icons/cursors.
- GNOME Keyring starts for the logged-in user through PAM/session integration and the Secret Service component is available.
- `xwayland-satellite` is installed and `/tmp` plus `/tmp/.X11-unix` are restored to `root:root` mode `1777` before the desktop starts.
- The direct-root mdev init no longer writes the obsolete `/proc/sys/kernel/hotplug`, and the Alpine BPF sysctl incompatible with this kernel is filtered from the generated test root.
- The test launcher passes `bash -n` and `git diff --check`.

Remaining warnings are intentionally limited to capability differences that are not desktop failures:
- Niri reports that the current Mesa virgl stack does not expose `EGL_WL_bind_wayland_display`.
- Niri probes the optional `org.freedesktop.locale1` service; the foundation uses OpenRC + elogind and does not ship systemd-localed.
- Noctalia cannot read a CPU thermal sensor from the QEMU guest. The guest has no hardware thermal zone to expose, so fabricating one would hide the limitation rather than solve it.

No fake locale service, thermal sensor, EGL shim, or second udev stack was added just to silence these messages.

The graphical desktop foundation milestone is complete. The next development step should move the currently cache-only Mesa virgl, Noctalia, greeter, and runtime-closure recipes into tracked repository tooling so a clean checkout can reproduce this environment without depending on `dist/.build`. After that, review which Alpine services remain external-system responsibilities and which boot/runtime responsibilities should transition to Luna-owned components. Keep the direct-root experiment isolated from `develop`.

## 2026-09-27 canonical Luna production boot — verified

The canonical builder now creates a minimal immutable System Image plus the real LUNA-SYS boot artifact set: System Image manifest, luna-init core and manifest, boot-state, and Recovery DATA Image with manifest. Niri and its musl closure remain DATA-owned.

Production OVMF verification now passes through UEFI, luna-boot, Luna kernel 7.2.4, memory-resident luna-init PID1, logical root preparation, luna-system-runtime, native UserSession and DRM/KMS login UI. The canonical Luna font and Luna artwork load from logical /resources. The disposable luna/luna credential authenticates and luna-system-runtime launches /usr/bin/niri --session. The source image checksum remains unchanged because tests use snapshot copies.

Fixes made during this milestone include idempotent kernel-provided devtmpfs handling, explicit /resources mapping, DATA musl loader mapping at /lib/ld-musl-x86_64.so.1, removal of the physical /luna-data user-home symlink, and production regression harnesses using virtio-gpu-pci.

Current limitation: D-Bus, NetworkManager, BlueZ and UDisks are not yet packaged in canonical DATA, so the runtime reports them as unavailable. Niri launches successfully with no immediate error during the validation window, but a longer-lived Wayland session regression remains to be added.

The direct-root Alpine desktop experiment remains the control path and is not promoted to `develop`.


## 2026-09-27 — external system provider closure milestone

The canonical LUNA-DATA provider layer now carries Alpine musl builds for the runtime system D-Bus daemon, NetworkManager and BlueZ daemon. luna-system-runtime starts all three from DATA in production OVMF; their executables are not copied into the boot-critical System Image.

Provider ownership remains external: executables live under LUNA-DATA/system/apps/<provider>/, shared runtime files under LUNA-DATA/system/libs/, and provider configuration under LUNA-DATA/system/config/. The runtime namespace exposes the daemons through /usr/bin without reintroducing the retired service-manager login path.

The canonical production OVMF regression was updated to assert these three provider-start markers and still passes image, PID1, logical-root, native login-renderer and no-panic checks. Login/session regression also passes after the provider addition: luna/luna authentication is accepted and /usr/bin/niri --session is launched.

UDisks2 is intentionally still pending. Its Alpine package is available, but the required block-device/polkit dependency closure is not present in the existing local cache; no host glibc binaries are used as a substitute. The remaining validation gap is a longer-lived Wayland/Niri session regression, plus completion of the UDisks2 musl provider closure.


## 2026-09-27 — sustained graphical session regression

The canonical login regression now keeps the QEMU guest alive for 20 seconds after injecting the development login credentials. The test requires successful authentication and Niri launch, and rejects graphical desktop exit, session-child start/wait failures, login UI I/O failure and kernel panic markers.

The fresh canonical image passes this sustained session check: luna/luna authentication is accepted, /usr/bin/niri --session is launched, and no graphical desktop exit is reported during the 20-second observation window. The structural canonical disk verification also passes on the same rebuilt image; GPT remains EFI / LUNA-SYS / LUNA-DATA / SWAP with the accepted partition sizes.
