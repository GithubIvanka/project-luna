#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$REPO_ROOT/dist"
WORK="$DIST/.build/canonical-luna-disk"
SYS_ROOT="$WORK/luna-sys"
DATA_ROOT="$WORK/luna-data"
PARTS="$WORK/parts"
SYSTEM_IMAGE_ROOT="$WORK/system-image-root"
RECOVERY_DATA_ROOT="$WORK/recovery-data-root"
ALPINE_ROOTFS="$DIST/sources/reference/alpine-3.24/release/alpine-runtime-rootfs-3.24.2-x86_64.tar.gz"
NIRI_CACHE="$DIST/.build/alpine-niri-apk-cache"
SERVICE_ROOT="$DIST/.build/alpine-system-service-root"
UDISKS_ROOT="$DIST/.build/udisks-provider-root/root"
MESA_ROOT="$DIST/.build/alpine-foundation-graphics/mesa/out"
NOCTALIA_BIN="$DIST/.build/noctalia-build/out/usr/local/bin/noctalia"
NOCTALIA_CACHE="$DIST/.build/noctalia-runtime-apks"
NOCTALIA_RUNTIME_ROOT="$DIST/.build/noctalia-runtime-root"
DESKTOP_SERVICE_ROOT="$WORK/desktop-service-root"
BUSYBOX_SOURCE="$SYS_ROOT/bin/busybox"
ADWAITA_APK="$DIST/.build/alpine-greeter-apks/packages/adwaita-icon-theme-50.0-r1.apk"
LIBUDEV_ZERO_APK="$DIST/.build/alpine-niri-apk-cache/libudev-zero-1.0.4-r0.apk"
ELOGIND_APK="$DIST/.build/alpine-greeter-apks/packages/elogind-255.24-r0.apk"
EUDEV_APK="$DIST/.build/alpine-niri-apk-cache/eudev-libs-3.2.14-r6.apk"
UDEV_DAEMON_APK="$DIST/.build/udev-provider-root/repo/eudev-3.2.14-r6.apk"
UDEV_REPO="$DIST/.build/udev-provider-root/repo"
UDEV_ROOT="$DIST/.build/udev-provider-root/root"
KERNEL="$DIST/kernel/current/bzImage"

LUNA_VERSION="${LUNA_VERSION:-0.1.0}"
VERSION="$LUNA_VERSION"
OUT="$DIST/luna-$VERSION.img"
EFI="$REPO_ROOT/components/boot/target/x86_64-unknown-uefi/release/luna-boot.efi"

for tool in fakeroot cargo rustup sgdisk mkfs.ext4 mkfs.fat mkswap mcopy mmd dd mksquashfs file truncate blkid; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
MKFS_BTRFS="$(command -v mkfs.btrfs || true)"
[ -n "$MKFS_BTRFS" ] || { echo "missing mkfs.btrfs" >&2; exit 1; }
[ -f "$ALPINE_ROOTFS" ] || { echo "missing Alpine rootfs: $ALPINE_ROOTFS" >&2; exit 1; }
[ -f "$KERNEL" ] || { echo "missing kernel: $KERNEL" >&2; exit 1; }
[ -d "$SERVICE_ROOT" ] || { echo "missing prepared Alpine system-service provider root: $SERVICE_ROOT" >&2; exit 1; }
[ -d "$UDISKS_ROOT/usr/lib" ] || { echo "missing prepared UDisks2 provider closure: $UDISKS_ROOT" >&2; exit 1; }
[ -d "$MESA_ROOT/usr/local/lib" ] || { echo "missing tracked Mesa virgl artifact: $MESA_ROOT" >&2; exit 1; }
[ -x "$NOCTALIA_BIN" ] || { echo "missing built Noctalia shell: $NOCTALIA_BIN" >&2; exit 1; }
[ -x "$BUSYBOX_SOURCE" ] || { echo "missing musl busybox source: $BUSYBOX_SOURCE" >&2; exit 1; }
[ -d "$NOCTALIA_CACHE" ] || { echo "missing Noctalia runtime APK cache: $NOCTALIA_CACHE" >&2; exit 1; }
[ -f "$ADWAITA_APK" ] || { echo "missing Adwaita icon theme APK: $ADWAITA_APK" >&2; exit 1; }
[ -f "$LIBUDEV_ZERO_APK" ] || { echo "missing libudev-zero APK: $LIBUDEV_ZERO_APK" >&2; exit 1; }
[ -f "$ELOGIND_APK" ] || { echo "missing elogind seat rules APK: $ELOGIND_APK" >&2; exit 1; }
[ -f "$EUDEV_APK" ] || { echo "missing eudev-libs APK: $EUDEV_APK" >&2; exit 1; }
[ -f "$UDEV_DAEMON_APK" ] || { echo "missing eudev daemon APK: $UDEV_DAEMON_APK" >&2; exit 1; }
[ -d "$UDEV_REPO" ] || { echo "missing udev provider repository: $UDEV_REPO" >&2; exit 1; }
rm -rf "$UDEV_ROOT"
mkdir -p "$UDEV_ROOT"
tar -xzf "$EUDEV_APK" -C "$UDEV_ROOT" usr/lib/libudev.so.1 usr/lib/libudev.so.1.6.3
for apk in "$UDEV_REPO"/*.apk; do
    tar -xzf "$apk" -C "$UDEV_ROOT" --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null || true
done

rm -rf "$WORK"
rm -rf "$DIST/luna-sys" "$DIST/luna-data"
rm -f "$DIST/luna-$VERSION.squashfs"
mkdir -p "$SYS_ROOT" "$DATA_ROOT" "$PARTS"

echo "==> Build luna-boot.efi"
cargo build --release --target x86_64-unknown-uefi --manifest-path "$REPO_ROOT/components/boot/Cargo.toml"
[ -f "$EFI" ] || { echo "missing luna-boot.efi" >&2; exit 1; }

echo "==> Base system -> LUNA-SYS"
tar -xzf "$ALPINE_ROOTFS" -C "$SYS_ROOT"

echo "==> Luna core artifacts"
cargo build --release -p luna-device-manager --target x86_64-unknown-linux-musl
cargo build --release -p luna-volume-manager --target x86_64-unknown-linux-musl
cargo build --release -p luna-system-runtime --target x86_64-unknown-linux-musl
cargo build --release --manifest-path "$REPO_ROOT/components/core/luna-init/Cargo.toml" --target x86_64-unknown-linux-musl
DEVICE_MANAGER="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-device-manager"
VOLUME_MANAGER="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-volume-manager"
RUNTIME="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/core/luna-init/target/x86_64-unknown-linux-musl/release/luna-init"
SESSION="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-user-session"
for artifact in "$DEVICE_MANAGER" "$VOLUME_MANAGER" "$RUNTIME" "$SESSION" "$LUNA_INIT"; do
    [ -x "$artifact" ] || { echo "missing core artifact: $artifact" >&2; exit 1; }
done
file "$DEVICE_MANAGER" "$VOLUME_MANAGER" "$RUNTIME" "$SESSION" "$LUNA_INIT" | grep -E 'static-pie linked|statically linked' >/dev/null || { echo "a required Luna core artifact is not statically linked" >&2; exit 1; }

echo "==> Luna UserSession -> LUNA-SYS"
cargo build --release -p luna-user-session --target x86_64-unknown-linux-musl
SESSION="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-user-session"
[ -x "$SESSION" ] || { echo "missing luna-user-session" >&2; exit 1; }
install -Dm0755 "$SESSION" "$SYS_ROOT/usr/bin/luna-user-session"

echo "==> Niri packages -> LUNA-SYS dependencies; provider -> LUNA-DATA"
mkdir -p "$DATA_ROOT/system"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$DATA_ROOT/users" "$DATA_ROOT/cach"
mkdir -p "$DATA_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
tar -xzf "$ADWAITA_APK" -C "$DATA_ROOT/system/resources/icons" --strip-components=3 usr/share/icons/Adwaita
find "$NIRI_CACHE" -maxdepth 1 -type f -name '*.apk' -print0 |
while IFS= read -r -d '' apk; do
    tar -xzf "$apk" -C "$SYS_ROOT" --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null || true
done
[ -x "$SYS_ROOT/usr/bin/niri" ] || { echo "Niri package missing" >&2; exit 1; }
mkdir -p "$DATA_ROOT/system/apps/niri" "$DATA_ROOT/system/libs/niri-compat"
mv "$SYS_ROOT/usr/bin/niri" "$DATA_ROOT/system/apps/niri/niri"
chmod 0755 "$DATA_ROOT/system/apps/niri/niri"
tar -xOf "$LIBUDEV_ZERO_APK" usr/lib/libudev.so.1 > "$DATA_ROOT/system/libs/niri-compat/libudev.so.1"

# The Niri provider is DATA-owned, including its musl runtime library closure.
# APKs are already resolved from the tracked local cache; keep only lib trees
# in the provider namespace and do not leak the Alpine host root into System Image.
NIRI_RUNTIME_ROOT="$WORK/niri-runtime-root"
rm -rf "$NIRI_RUNTIME_ROOT"
mkdir -p "$NIRI_RUNTIME_ROOT"
find "$NIRI_CACHE" -maxdepth 1 -type f -name '*.apk' -print0 |
while IFS= read -r -d '' apk; do
    tar -xzf "$apk" -C "$NIRI_RUNTIME_ROOT" --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null || true
done
if [ -d "$NIRI_RUNTIME_ROOT/lib" ]; then
    cp -a "$NIRI_RUNTIME_ROOT/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$NIRI_RUNTIME_ROOT/usr/lib" ]; then
    cp -a "$NIRI_RUNTIME_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
fi
rm -rf "$NOCTALIA_RUNTIME_ROOT"
mkdir -p "$NOCTALIA_RUNTIME_ROOT"
find "$NOCTALIA_CACHE" -maxdepth 1 -type f -name '*.apk' -print0 |
while IFS= read -r -d '' apk; do
    tar -xzf "$apk" -C "$NOCTALIA_RUNTIME_ROOT" --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null || true
done
if [ -d "$NOCTALIA_RUNTIME_ROOT/usr/lib" ]; then
    cp -a "$NOCTALIA_RUNTIME_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$NOCTALIA_RUNTIME_ROOT/lib" ]; then
    cp -a "$NOCTALIA_RUNTIME_ROOT/lib/." "$DATA_ROOT/system/libs/"
fi
mkdir -p "$DATA_ROOT/system/apps/noctalia"
install -Dm0755 "$NOCTALIA_BIN" "$DATA_ROOT/system/apps/noctalia/noctalia"
mkdir -p "$DATA_ROOT/system/apps/busybox"
install -Dm0755 "$BUSYBOX_SOURCE" "$DATA_ROOT/system/apps/busybox/busybox"
# Keep the normal Alpine BusyBox applet namespace available to OpenRC and
# session scripts. The canonical runtime still exposes the single BusyBox ELF;
# these are only applet links, not duplicated binaries.
for busybox_dir in "$SYS_ROOT/bin" "$SYS_ROOT/sbin" "$SYS_ROOT/usr/bin"; do
    if [ -d "$busybox_dir" ]; then
        for applet in "$busybox_dir"/*; do
            [ -L "$applet" ] || continue
            target="$(readlink "$applet")"
            case "$target" in
                busybox|/bin/busybox|/usr/bin/busybox)
                    name="$(basename "$applet")"
                    [ "$name" = "busybox" ] || ln -sfn busybox "$DATA_ROOT/system/apps/busybox/$name"
                    ;;
            esac
        done
    fi
done

echo "==> Restore OpenRC-managed desktop lifecycle -> LUNA-DATA"
rm -rf "$DESKTOP_SERVICE_ROOT"
mkdir -p "$DESKTOP_SERVICE_ROOT"
desktop_service_apks=(
    dbus-1.16.2-r2.apk
    dbus-daemon-launch-helper-1.16.2-r2.apk
    dbus-libs-1.16.2-r2.apk
    dbus-openrc-1.16.2-r2.apk
    elogind-255.24-r0.apk
    elogind-common-255.24-r0.apk
    elogind-openrc-255.24-r0.apk
    upower-1.91.0-r2.apk
    bluez-5.86-r2.apk
    bluez-libs-5.86-r2.apk
    bluez-openrc-5.86-r2.apk
    networkmanager-1.52.2-r0.apk
    networkmanager-cli-1.52.2-r0.apk
    networkmanager-openrc-1.52.2-r0.apk
    polkit-127-r2.apk
    polkit-common-127-r2.apk
    polkit-elogind-127-r2.apk
    polkit-elogind-libs-127-r2.apk
    polkit-openrc-127-r2.apk
    rtkit-0.14-r0.apk
    rtkit-openrc-0.14-r0.apk
    pipewire-1.6.8-r4.apk
    pipewire-libs-1.6.8-r4.apk
    pipewire-spa-bluez-1.6.8-r4.apk
    wireplumber-0.5.15-r0.apk
    wireplumber-libs-0.5.15-r0.apk
    wireplumber-logind-0.5.15-r0.apk
)
for apk_name in "${desktop_service_apks[@]}"; do
    apk="$DIST/.build/alpine-greeter-apks/packages/$apk_name"
    if [ ! -s "$apk" ]; then
        apk="$NOCTALIA_CACHE/$apk_name"
    fi
    [ -s "$apk" ] || { echo "missing desktop lifecycle APK: $apk_name" >&2; exit 1; }
    tar -xzf "$apk" -C "$DESKTOP_SERVICE_ROOT" --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null
done

mkdir -p "$DATA_ROOT/system/apps/openrc/resources/rc"
for openrc_bin in openrc openrc-run rc-service rc-sstat rc-update start-stop-daemon supervise-daemon; do
    install -Dm0755 "$SYS_ROOT/sbin/$openrc_bin"         "$DATA_ROOT/system/apps/openrc/$openrc_bin"
done
cp -a "$SYS_ROOT/usr/libexec/rc/." "$DATA_ROOT/system/apps/openrc/resources/rc/"
cp -a "$SYS_ROOT/usr/lib/librc.so.1"* "$DATA_ROOT/system/libs/"
cp -a "$SYS_ROOT/usr/lib/libeinfo.so.1"* "$DATA_ROOT/system/libs/"
cp -a "$SYS_ROOT/etc/rc.conf" "$DATA_ROOT/system/config/rc.conf"
mkdir -p "$DATA_ROOT/system/config/init.d" "$DATA_ROOT/system/config/conf.d"     "$DATA_ROOT/system/config/local.d" "$DATA_ROOT/system/config/runlevels"/{sysinit,boot,default,nonetwork,shutdown}     "$DATA_ROOT/system/config/user/init.d" "$DATA_ROOT/system/config/user/conf.d"
cp -a "$SYS_ROOT/etc/init.d/cgroups" "$DATA_ROOT/system/config/init.d/cgroups"
cp -a "$SYS_ROOT/etc/init.d/local" "$DATA_ROOT/system/config/init.d/local"
cp -a "$SYS_ROOT/etc/rc.conf" "$DATA_ROOT/system/config/rc.conf"
cp -a "$SYS_ROOT/etc/user/." "$DATA_ROOT/system/config/user/" 2>/dev/null || true

cat > "$DATA_ROOT/system/config/init.d/luna-device-manager" <<'EOF_OPENRC_LUNA_DEVICE'
#!/sbin/openrc-run
name="Luna Device Manager"
description="Physical device discovery and kernel hotplug events"
command="/usr/bin/luna-device-manager"
command_background="yes"
pidfile="/run/luna-device-manager.pid"
depend() { provide luna-device-manager; }
EOF_OPENRC_LUNA_DEVICE
chmod 0755 "$DATA_ROOT/system/config/init.d/luna-device-manager"

cat > "$DATA_ROOT/system/config/init.d/luna-volume-manager" <<'EOF_OPENRC_LUNA_VOLUME'
#!/sbin/openrc-run
name="Luna Volume Manager"
description="Logical volumes and automatic availability"
command="/usr/bin/luna-volume-manager"
command_background="yes"
pidfile="/run/luna-volume-manager.pid"
depend() { need luna-device-manager; after luna-device-manager; }
EOF_OPENRC_LUNA_VOLUME
chmod 0755 "$DATA_ROOT/system/config/init.d/luna-volume-manager"

cat > "$DATA_ROOT/system/config/init.d/seatd" <<'EOF_OPENRC_SEATD'
#!/sbin/openrc-run
name="seatd"
description="Seat management daemon"
command="/usr/bin/seatd"
command_args="-g seat -l info"
command_background="yes"
pidfile="/run/seatd.pid"
depend() { provide seatd; }
EOF_OPENRC_SEATD
chmod 0755 "$DATA_ROOT/system/config/init.d/seatd"

cat > "$DATA_ROOT/system/config/init.d/dbus" <<'EOF_OPENRC_DBUS'
#!/sbin/openrc-run
name="System Message Bus"
description="D-Bus System Message Bus"
command="/usr/bin/dbus-daemon"
command_args="--system --nofork --nopidfile"
command_background="yes"
pidfile="/run/dbus.pid"
start_pre() {
    mkdir -p /run/dbus
    /usr/bin/dbus-uuidgen --ensure=/etc/machine-id || true
}
depend() { provide dbus; }
EOF_OPENRC_DBUS
chmod 0755 "$DATA_ROOT/system/config/init.d/dbus"

cat > "$DATA_ROOT/system/config/init.d/elogind" <<'EOF_OPENRC_ELOGIND'
#!/sbin/openrc-run
name="System login manager"
description="elogind session and seat manager"
command="/usr/bin/elogind"
command_background="yes"
pidfile="/run/elogind.pid"
depend() { need dbus cgroups; before display-manager; }
EOF_OPENRC_ELOGIND
chmod 0755 "$DATA_ROOT/system/config/init.d/elogind"

cat > "$DATA_ROOT/system/config/init.d/upower" <<'EOF_OPENRC_UPOWER'
#!/sbin/openrc-run
name="UPower"
description="Power management daemon"
command="/usr/bin/upowerd"
command_background="yes"
pidfile="/run/upowerd.pid"
depend() { need dbus; after elogind; }
EOF_OPENRC_UPOWER
chmod 0755 "$DATA_ROOT/system/config/init.d/upower"

cat > "$DATA_ROOT/system/config/init.d/networkmanager" <<'EOF_OPENRC_NM'
#!/sbin/openrc-run
name="NetworkManager"
description="Network Manager Daemon"
command="/usr/bin/NetworkManager"
command_args="-n"
command_background="yes"
pidfile="/run/NetworkManager.pid"
depend() { need dbus; provide net; }
EOF_OPENRC_NM
chmod 0755 "$DATA_ROOT/system/config/init.d/networkmanager"

cat > "$DATA_ROOT/system/config/init.d/bluetooth" <<'EOF_OPENRC_BT'
#!/sbin/openrc-run
name="Bluetooth"
description="Bluetooth daemon"
command="/usr/bin/bluetoothd"
command_background="yes"
pidfile="/run/bluetoothd.pid"
depend() { need dbus; after elogind; }
EOF_OPENRC_BT
chmod 0755 "$DATA_ROOT/system/config/init.d/bluetooth"

cat > "$DATA_ROOT/system/config/init.d/polkit" <<'EOF_OPENRC_POLKIT'
#!/sbin/openrc-run
name="Polkit System Daemon"
description="PolicyKit authorization daemon"
command="/usr/bin/polkitd"
command_args="--no-debug"
command_background="yes"
pidfile="/run/polkitd.pid"
depend() { need dbus; }
EOF_OPENRC_POLKIT
chmod 0755 "$DATA_ROOT/system/config/init.d/polkit"

cat > "$DATA_ROOT/system/config/init.d/rtkit" <<'EOF_OPENRC_RTKIT'
#!/sbin/openrc-run
name="RealtimeKit"
description="Realtime scheduling policy daemon"
command="/usr/bin/rtkit-daemon"
command_background="yes"
pidfile="/run/rtkit-daemon.pid"
depend() { need dbus; }
EOF_OPENRC_RTKIT
chmod 0755 "$DATA_ROOT/system/config/init.d/rtkit"

cat > "$DATA_ROOT/system/config/init.d/luna-session" <<'EOF_OPENRC_SESSION'
#!/sbin/openrc-run
name="Luna UserSession"
description="Native Luna graphical login and user session"
command="/usr/bin/luna-user-session"
command_args="--login-launch /usr/bin/luna-graphical-session"
command_background="yes"
pidfile="/run/luna-session.pid"
respawn_delay=2
respawn_max=0
depend() {
    need dbus elogind seatd
    provide display-manager
}
EOF_OPENRC_SESSION
chmod 0755 "$DATA_ROOT/system/config/init.d/luna-session"

ln -sfn ../../init.d/cgroups "$DATA_ROOT/system/config/runlevels/sysinit/cgroups"
ln -sfn ../../init.d/seatd "$DATA_ROOT/system/config/runlevels/boot/seatd"
for svc in dbus elogind upower bluetooth networkmanager polkit rtkit luna-device-manager luna-volume-manager; do
    ln -sfn ../../init.d/"$svc" "$DATA_ROOT/system/config/runlevels/default/$svc"
done

cat > "$DATA_ROOT/system/config/user/init.d/dbus" <<'EOF_OPENRC_USER_DBUS'
#!/sbin/openrc-run
name="Login Message Bus"
description="D-Bus Login Session Message Bus"
supervisor=supervise-daemon
command="/usr/bin/dbus-daemon"
command_args="--session --nofork --nopidfile --address=$DBUS_SESSION_BUS_ADDRESS --print-address=3"
notify=fd:3
EOF_OPENRC_USER_DBUS
chmod 0755 "$DATA_ROOT/system/config/user/init.d/dbus"
cat > "$DATA_ROOT/system/config/user/conf.d/dbus" <<'EOF_OPENRC_USER_DBUS_CONF'
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
EOF_OPENRC_USER_DBUS_CONF

# Desktop daemons are external providers. Their binaries are exposed through
# the normal DATA application namespace; OpenRC owns their system lifecycle,
# while PipeWire/WirePlumber remain session children of luna-graphical-session.
mkdir -p "$DATA_ROOT/system/apps/elogind" "$DATA_ROOT/system/apps/upower"     "$DATA_ROOT/system/apps/polkit" "$DATA_ROOT/system/apps/pipewire"     "$DATA_ROOT/system/apps/wireplumber" "$DATA_ROOT/system/apps/rtkit"

install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/libexec/elogind/elogind"     "$DATA_ROOT/system/apps/elogind/elogind"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/loginctl"     "$DATA_ROOT/system/apps/elogind/loginctl"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/libexec/upowerd"     "$DATA_ROOT/system/apps/upower/upowerd"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/lib/polkit-1/polkitd"     "$DATA_ROOT/system/apps/polkit/polkitd"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/pipewire"     "$DATA_ROOT/system/apps/pipewire/pipewire"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/wireplumber"     "$DATA_ROOT/system/apps/wireplumber/wireplumber"
install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/libexec/rtkit-daemon"     "$DATA_ROOT/system/apps/rtkit/rtkit-daemon"

if [ -f "$DESKTOP_SERVICE_ROOT/usr/libexec/dbus-daemon-launch-helper" ]; then
    mkdir -p "$DATA_ROOT/system/apps/dbus-daemon"
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/libexec/dbus-daemon-launch-helper"         "$DATA_ROOT/system/apps/dbus-daemon/dbus-daemon-launch-helper"
fi
if [ -x "$DESKTOP_SERVICE_ROOT/usr/bin/dbus-run-session" ]; then
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/dbus-run-session"         "$DATA_ROOT/system/apps/dbus-daemon/dbus-run-session"
fi
for dbus_client in dbus-send dbus-monitor dbus-update-activation-environment; do
    if [ -x "$DESKTOP_SERVICE_ROOT/usr/bin/$dbus_client" ]; then
        install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/$dbus_client"             "$DATA_ROOT/system/apps/dbus-daemon/$dbus_client"
    fi
done
if [ -x "$DESKTOP_SERVICE_ROOT/usr/bin/dbus-uuidgen" ]; then
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/dbus-uuidgen"         "$DATA_ROOT/system/apps/dbus-daemon/dbus-uuidgen"
fi
if [ -x "$DESKTOP_SERVICE_ROOT/usr/sbin/NetworkManager" ]; then
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/sbin/NetworkManager"         "$DATA_ROOT/system/apps/networkmanager/NetworkManager"
elif [ -x "$DESKTOP_SERVICE_ROOT/usr/bin/NetworkManager" ]; then
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/bin/NetworkManager"         "$DATA_ROOT/system/apps/networkmanager/NetworkManager"
fi
if [ -x "$DESKTOP_SERVICE_ROOT/usr/lib/bluetooth/bluetoothd" ]; then
    install -Dm0755 "$DESKTOP_SERVICE_ROOT/usr/lib/bluetooth/bluetoothd"         "$DATA_ROOT/system/apps/bluez/bluetoothd"
fi
install -Dm0755 "$UDEV_ROOT/usr/lib/libudev.so.1.6.3" "$DATA_ROOT/system/libs/libudev.so.1.6.3"
if [ -d "$DESKTOP_SERVICE_ROOT/usr/lib" ]; then
    cp -a "$DESKTOP_SERVICE_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$DESKTOP_SERVICE_ROOT/lib" ]; then
    cp -a "$DESKTOP_SERVICE_ROOT/lib/." "$DATA_ROOT/system/libs/"
fi
for provider_config in dbus-1 elogind NetworkManager bluetooth polkit-1 upower; do
    if [ -d "$DESKTOP_SERVICE_ROOT/etc/$provider_config" ]; then
        mkdir -p "$DATA_ROOT/system/config/$(dirname "$provider_config")"
        cp -a "$DESKTOP_SERVICE_ROOT/etc/$provider_config" "$DATA_ROOT/system/config/$provider_config"
    fi
done

mkdir -p "$DATA_ROOT/system/apps/pipewire/resources"     "$DATA_ROOT/system/apps/wireplumber/resources"
cat > "$DATA_ROOT/system/apps/noctalia/luna-noctalia-test" <<'EOF_NOCTALIA_WRAPPER'
#!/bin/sh
export NOCTALIA_LOG_LEVEL=debug
exec /usr/bin/noctalia >>"${XDG_RUNTIME_DIR:-/tmp}/luna-noctalia.log" 2>&1
EOF_NOCTALIA_WRAPPER
chmod 0755 "$DATA_ROOT/system/apps/noctalia/luna-noctalia-test"
if [ -d "$DESKTOP_SERVICE_ROOT/usr/share/pipewire" ]; then
    cp -a "$DESKTOP_SERVICE_ROOT/usr/share/pipewire"         "$DATA_ROOT/system/apps/pipewire/resources/pipewire"
fi
if [ -d "$DESKTOP_SERVICE_ROOT/usr/share/wireplumber" ]; then
    cp -a "$DESKTOP_SERVICE_ROOT/usr/share/wireplumber"         "$DATA_ROOT/system/apps/wireplumber/resources/wireplumber"
fi
if [ -d "$DESKTOP_SERVICE_ROOT/usr/share/dbus-1" ]; then
    mkdir -p "$DATA_ROOT/system/apps/dbus-daemon/resources"
    cp -a "$DESKTOP_SERVICE_ROOT/usr/share/dbus-1/."         "$DATA_ROOT/system/apps/dbus-daemon/resources/dbus-1/"
fi
if [ -d "$DESKTOP_SERVICE_ROOT/usr/share/polkit-1" ]; then
    mkdir -p "$DATA_ROOT/system/apps/dbus-daemon/resources/polkit-1"
    cp -a "$DESKTOP_SERVICE_ROOT/usr/share/polkit-1/."         "$DATA_ROOT/system/apps/dbus-daemon/resources/polkit-1/"
fi

if [ -L "$UDEV_ROOT/usr/lib/libudev.so.1" ]; then
    rm -f "$DATA_ROOT/system/libs/libudev.so.1"
    cp -a "$UDEV_ROOT/usr/lib/libudev.so.1" "$DATA_ROOT/system/libs/libudev.so.1"
fi
if [ -d "$MESA_ROOT/usr/local/lib" ]; then
    cp -a "$MESA_ROOT/usr/local/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$MESA_ROOT/usr/lib/dri" ]; then
    mkdir -p "$DATA_ROOT/system/libs/dri"
    cp -a "$MESA_ROOT/usr/lib/dri/." "$DATA_ROOT/system/libs/dri/"
fi
if [ -x "$NIRI_RUNTIME_ROOT/usr/bin/seatd" ]; then
    mkdir -p "$DATA_ROOT/system/apps/seatd"
    install -Dm0755 "$NIRI_RUNTIME_ROOT/usr/bin/seatd" "$DATA_ROOT/system/apps/seatd/seatd"
fi
if [ -d "$NIRI_RUNTIME_ROOT/etc/fonts" ]; then
    mkdir -p "$DATA_ROOT/system/config/fonts"
    cp -a "$NIRI_RUNTIME_ROOT/etc/fonts/." "$DATA_ROOT/system/config/fonts/"
fi
for niri_resource in libinput xkeyboard-config-2 X11; do
    source_resource="$NIRI_RUNTIME_ROOT/usr/share/$niri_resource"
    if [ -e "$source_resource" ]; then
        mkdir -p "$DATA_ROOT/system/apps/niri/resources"
        rm -rf "$DATA_ROOT/system/apps/niri/resources/$niri_resource"
        if [ "$niri_resource" = "xkeyboard-config-2" ]; then
            cp -aL "$source_resource" "$DATA_ROOT/system/apps/niri/resources/$niri_resource"
        else
            cp -a "$source_resource" "$DATA_ROOT/system/apps/niri/resources/$niri_resource"
        fi
    fi
done

echo "==> External system providers -> LUNA-DATA"
mkdir -p "$DATA_ROOT/system/apps/udevd/resources/udev/rules.d" "$DATA_ROOT/system/apps/dbus-daemon/resources" "$DATA_ROOT/system/apps/networkmanager" "$DATA_ROOT/system/apps/bluez" "$DATA_ROOT/system/apps/udisksd/resources" "$DATA_ROOT/system/apps/udisksctl"
install -Dm0755 "$UDEV_ROOT/sbin/udevd" "$DATA_ROOT/system/apps/udevd/udevd"
install -Dm0755 "$UDEV_ROOT/bin/udevadm" "$DATA_ROOT/system/apps/udevd/udevadm"
cp -a "$UDEV_ROOT/usr/lib/udev/." "$DATA_ROOT/system/apps/udevd/resources/udev/"
tar -xOf "$ELOGIND_APK" usr/lib/udev/rules.d/71-seat.rules > "$DATA_ROOT/system/apps/udevd/resources/udev/rules.d/71-seat.rules"
tar -xOf "$ELOGIND_APK" usr/lib/udev/rules.d/73-seat-late.rules > "$DATA_ROOT/system/apps/udevd/resources/udev/rules.d/73-seat-late.rules"
cat > "$DATA_ROOT/system/apps/udevd/resources/udev/rules.d/72-luna-seat.rules" <<'EOF_LUNA_SEAT_RULES'
SUBSYSTEM=="input", KERNEL=="input[0-9]*", TAG+="seat", ENV{ID_SEAT}="seat0"
SUBSYSTEM=="input", KERNEL=="event[0-9]*", TAG+="seat", ENV{ID_SEAT}="seat0"
SUBSYSTEM=="drm", KERNEL=="card[0-9]*", TAG+="seat", TAG+="master-of-seat", ENV{ID_SEAT}="seat0"
EOF_LUNA_SEAT_RULES
install -Dm0755 "$SERVICE_ROOT/usr/bin/dbus-daemon" "$DATA_ROOT/system/apps/dbus-daemon/dbus-daemon"
install -Dm0755 "$SERVICE_ROOT/usr/sbin/NetworkManager" "$DATA_ROOT/system/apps/networkmanager/NetworkManager"
install -Dm0755 "$SERVICE_ROOT/usr/lib/bluetooth/bluetoothd" "$DATA_ROOT/system/apps/bluez/bluetoothd"
install -Dm0755 "$UDISKS_ROOT/usr/libexec/udisks2/udisksd" "$DATA_ROOT/system/apps/udisksd/udisksd"
install -Dm0755 "$UDISKS_ROOT/usr/bin/udisksctl" "$DATA_ROOT/system/apps/udisksctl/udisksctl"
if [ -d "$SERVICE_ROOT/usr/lib" ]; then
    cp -a "$SERVICE_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$SERVICE_ROOT/lib" ]; then
    cp -a "$SERVICE_ROOT/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$UDISKS_ROOT/usr/lib" ]; then
    cp -a "$UDISKS_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
fi
if [ -d "$SERVICE_ROOT/usr/share/dbus-1" ]; then
    cp -a "$SERVICE_ROOT/usr/share/dbus-1" "$DATA_ROOT/system/apps/dbus-daemon/resources/dbus-1"
fi
if [ -d "$SERVICE_ROOT/usr/share/polkit-1" ]; then
    cp -a "$SERVICE_ROOT/usr/share/polkit-1" "$DATA_ROOT/system/apps/dbus-daemon/resources/polkit-1"
fi
if [ -d "$UDISKS_ROOT/usr/lib/udisks2" ]; then
    cp -a "$UDISKS_ROOT/usr/lib/udisks2" "$DATA_ROOT/system/apps/udisksd/resources/udisks2"
fi
if [ -d "$UDISKS_ROOT/usr/share/dbus-1" ]; then
    rm -rf "$DATA_ROOT/system/apps/udisksd/resources/dbus-1"
    cp -a "$UDISKS_ROOT/usr/share/dbus-1" "$DATA_ROOT/system/apps/udisksd/resources/dbus-1"
    if [ -f "$DATA_ROOT/system/apps/udisksd/resources/dbus-1/system-services/org.freedesktop.UDisks2.service" ]; then
        sed -i 's#/usr/libexec/udisks2/udisksd#/usr/bin/udisksd#g' "$DATA_ROOT/system/apps/udisksd/resources/dbus-1/system-services/org.freedesktop.UDisks2.service"
    fi
fi
if [ -d "$UDISKS_ROOT/usr/share/polkit-1" ]; then
    rm -rf "$DATA_ROOT/system/apps/udisksd/resources/polkit-1"
    cp -a "$UDISKS_ROOT/usr/share/polkit-1" "$DATA_ROOT/system/apps/udisksd/resources/polkit-1"
fi
for provider_config in dbus-1 NetworkManager bluetooth udisks2 libblockdev/3 udev; do
    provider_root="$SERVICE_ROOT"
    case "$provider_config" in
        udisks2|libblockdev/3) provider_root="$UDISKS_ROOT" ;;
        udev) provider_root="$UDEV_ROOT" ;;
    esac
    if [ -d "$provider_root/etc/$provider_config" ]; then
        mkdir -p "$DATA_ROOT/system/config/$(dirname "$provider_config")"
        cp -a "$provider_root/etc/$provider_config" "$DATA_ROOT/system/config/$provider_config"
    fi
done

echo "==> User content -> LUNA-DATA"
bash "$REPO_ROOT/components/boot/tests/ovmf/configure-alpine-test-user.sh" "$SYS_ROOT"
if grep -q "^seat:" "$SYS_ROOT/etc/group"; then
    sed -i -E "s/^(seat:x:[^:]*:).*/\1luna/" "$SYS_ROOT/etc/group"
else
    printf "%s\n" "seat:x:83:luna" >> "$SYS_ROOT/etc/group"
fi
if grep -q "^input:" "$SYS_ROOT/etc/group"; then
    sed -i -E "s/^(input:x:[^:]*:).*/\1luna/" "$SYS_ROOT/etc/group"
else
    printf "%s\n" "input:x:23:luna" >> "$SYS_ROOT/etc/group"
fi
mkdir -p "$DATA_ROOT/users/luna/home" "$DATA_ROOT/users/luna/data" "$DATA_ROOT/users/luna/config/niri" "$DATA_ROOT/users/luna/config/noctalia"
cp -a "$SYS_ROOT/home/luna/." "$DATA_ROOT/users/luna/home/"
rm -rf "$SYS_ROOT/home/luna"
cat > "$DATA_ROOT/users/luna/config/niri/config.kdl" <<'EOF_NIRI'
input {
    keyboard {
        xkb {
            layout "us"
        }
    }
}
cursor {
    xcursor-theme "Adwaita"
    xcursor-size 24
}
layout {}
hotkey-overlay {
    skip-at-startup
}
spawn-at-startup "/usr/bin/luna-noctalia-test"
EOF_NIRI
cat > "$DATA_ROOT/users/luna/config/noctalia/00-luna-qemu.toml" <<'EOF_NOCTALIA_QEMU'
[system.monitor]
enabled = false
EOF_NOCTALIA_QEMU
cat > "$DATA_ROOT/users/luna/config/noctalia/01-luna-session.toml" <<'EOF_NOCTALIA_SESSION'
[shell]
polkit_agent = false

[shell.session.power]
reboot = "loginctl reboot"
shutdown = "loginctl poweroff"
EOF_NOCTALIA_SESSION

cat > "$SYS_ROOT/etc/init.d/seatd" <<"EOF_SEATD_INIT"
#!/sbin/openrc-run
name="seatd"
description="Seat management daemon for the graphical session"
command="/usr/bin/seatd"
command_args="-g seat"
command_background="yes"
pidfile="/run/seatd.pid"
depend() { need localmount; provide seatd; }
EOF_SEATD_INIT
chmod 0755 "$SYS_ROOT/etc/init.d/seatd"

echo "==> Add transitional DATA mount/session plumbing to LUNA-SYS"
mkdir -p "$SYS_ROOT/etc/runlevels/default" "$SYS_ROOT/luna-data"
cat > "$SYS_ROOT/etc/luna-data-partuuid" <<'EOF_GUID'
__DATA_GUID__
EOF_GUID
cat > "$SYS_ROOT/etc/init.d/luna-data" <<'EOF_DATA_INIT'
#!/sbin/openrc-run
name="Luna DATA"
description="Mount canonical LUNA-DATA"
depend() {
    need localmount
    provide luna-data
}
start() {
    ebegin "Mounting LUNA-DATA"
    mkdir -p /luna-data
    guid="$(cat /etc/luna-data-partuuid)"
    mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
    dev=""
    if [ ! -b /dev/sda3 ]; then /bin/mknod -m 0660 /dev/sda3 b 8 3 2>/dev/null || true; fi
    if [ -b /dev/sda3 ]; then dev=/dev/sda3; fi
    if [ ! -b /dev/sda3 ]; then mknod -m 0660 /dev/sda3 b 8 3 2>/dev/null || true; fi
    /sbin/mdev -s >/dev/null 2>&1 || true
    for dev_file in /sys/class/block/*/dev; do
        [ -r "$dev_file" ] || continue
        blockdir="$(dirname "$dev_file")"
        name="$(basename "$blockdir")"
        node="/dev/$name"
        if [ ! -b "$node" ]; then
            major="$(cut -d: -f1 "$dev_file")"
            minor="$(cut -d: -f2 "$dev_file")"
            mknod -m 0660 "$node" b "$major" "$minor" 2>/dev/null || true
        fi
    done
    for candidate in /dev/sd?* /dev/vd?* /dev/xvd?* /dev/nvme*n*; do
        [ -b "$candidate" ] || continue
        candidate_guid="$(blkid -p -s PARTUUID -o value "$candidate" 2>/dev/null || true)"
        candidate_label="$(blkid -p -s LABEL -o value "$candidate" 2>/dev/null || true)"
        if [ "$candidate_guid" = "$guid" ] || [ -z "$candidate_guid" ] && [ "$candidate_label" = "LUNA-DATA" ]; then
            dev="$candidate"
            break
        fi
    done
    if [ -z "$dev" ]; then
        root_source="$(findmnt -n -o SOURCE / 2>/dev/null || true)"
        case "$root_source" in
            */*p2) data_candidate="${root_source%p2}p3" ;;
            */*2) data_candidate="${root_source%2}3" ;;
            *) data_candidate="" ;;
        esac
        if [ -n "$data_candidate" ] && [ ! -b "$data_candidate" ]; then
            data_name="${data_candidate#/dev/}"
            data_dev_file="/sys/class/block/$data_name/dev"
            if [ -r "$data_dev_file" ]; then
                data_major="$(cut -d: -f1 "$data_dev_file")"
                data_minor="$(cut -d: -f2 "$data_dev_file")"
                mknod -m 0660 "$data_candidate" b "$data_major" "$data_minor" 2>/dev/null || true
            fi
        fi
        if [ -b "$data_candidate" ]; then
            candidate_label="$(blkid -p -s LABEL -o value "$data_candidate" 2>/dev/null || true)"
            [ "$candidate_label" = "LUNA-DATA" ] && dev="$data_candidate"
        fi
    fi
    [ -n "$dev" ] || { eerror "LUNA-DATA not found for PARTUUID=$guid"; return 1; }
    mount -t btrfs "$dev" /luna-data || return 1
    mkdir -p /home/luna
    mount --bind /luna-data/users/luna/home /home/luna || return 1
    eend 0
}
stop() {
    umount /home/luna 2>/dev/null || true
    umount /luna-data 2>/dev/null || true
}
EOF_DATA_INIT
chmod 0755 "$SYS_ROOT/etc/init.d/luna-data"

cat > "$SYS_ROOT/usr/local/bin/luna-native-session" <<'EOF_SESSION'
#!/bin/sh
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export LIBSEAT_BACKEND=seatd
exec /usr/bin/luna-user-session --login-launch /usr/local/bin/luna-graphical-session
EOF_SESSION
chmod 0755 "$SYS_ROOT/usr/local/bin/luna-native-session"

cat > "$SYS_ROOT/usr/local/bin/luna-graphical-session" <<'EOF_GRAPHICS'
#!/bin/sh
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export LIBSEAT_BACKEND=seatd
exec /luna-data/system/apps/niri/niri --session
EOF_GRAPHICS
chmod 0755 "$SYS_ROOT/usr/local/bin/luna-graphical-session"

cat > "$SYS_ROOT/etc/init.d/luna-session" <<'EOF_SESSION_INIT'
#!/sbin/openrc-run
name="Luna UserSession"
command="/usr/local/bin/luna-native-session"
command_background="yes"
pidfile="/run/luna-session.pid"
respawn_delay=2
respawn_max=0
depend() {
    need luna-data seatd
    provide display-manager
}
EOF_SESSION_INIT
chmod 0755 "$SYS_ROOT/etc/init.d/luna-session"
for svc in dbus elogind luna-data luna-session; do
    [ -e "$SYS_ROOT/etc/init.d/$svc" ] && ln -sfn "/etc/init.d/$svc" "$SYS_ROOT/etc/runlevels/default/$svc"
done

mkdir -p "$SYS_ROOT/resources/fonts" "$SYS_ROOT/resources/images"
install -Dm0644 "$REPO_ROOT/dist/nerd-fonts-root/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf" "$SYS_ROOT/resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
install -Dm0644 "$REPO_ROOT/docs/design/visual-concepts/luna.png" "$SYS_ROOT/resources/images/luna.png"

echo "==> Canonical boot artifacts -> LUNA-SYS"
mkdir -p "$SYS_ROOT/kernels/7.2.4" "$SYS_ROOT/images" "$SYS_ROOT/config" "$SYS_ROOT/cores" "$SYS_ROOT/recovery" "$SYS_ROOT/alpine"
cp "$KERNEL" "$SYS_ROOT/kernels/7.2.4/bzImage"
cp "$KERNEL" "$SYS_ROOT/alpine/luna-kernel"

INIT_VERSION="$VERSION"
cp "$LUNA_INIT" "$SYS_ROOT/cores/luna-$INIT_VERSION.init"
chmod 0755 "$SYS_ROOT/cores/luna-$INIT_VERSION.init"
cat > "$SYS_ROOT/cores/luna-$INIT_VERSION.toml" <<EOF_INIT
[init]
name = "luna-init"
version = "$INIT_VERSION"

[architecture]
arch = "x86_64"

[kernels]
compatible = ["7.2.4"]
EOF_INIT

cat > "$SYS_ROOT/config/boot-state.toml" <<EOF_BOOTSTATE
[state]
format = 1
generation = 1

[targets.current]
image = "$VERSION"
init = "$INIT_VERSION"
kernel = "7.2.4"

[targets.factory]
image = "$VERSION"
init = "$INIT_VERSION"
kernel = "7.2.4"

[targets.recovery]
image = "$VERSION"
init = "$INIT_VERSION"
kernel = "7.2.4"
EOF_BOOTSTATE

rm -rf "$SYSTEM_IMAGE_ROOT"
mkdir -p "$SYSTEM_IMAGE_ROOT"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$SYSTEM_IMAGE_ROOT/config/luna" "$SYSTEM_IMAGE_ROOT/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}

install -Dm0755 "$DEVICE_MANAGER" "$SYSTEM_IMAGE_ROOT/apps/luna-device-manager/luna-device-manager"
install -Dm0755 "$VOLUME_MANAGER" "$SYSTEM_IMAGE_ROOT/apps/luna-volume-manager/luna-volume-manager"
install -Dm0755 "$RUNTIME" "$SYSTEM_IMAGE_ROOT/apps/luna-system-runtime/luna-system-runtime"
install -Dm0755 "$SESSION" "$SYSTEM_IMAGE_ROOT/apps/luna-user-session/luna-user-session"
mkdir -p "$SYSTEM_IMAGE_ROOT/apps/luna-graphical-session"
cat > "$SYSTEM_IMAGE_ROOT/apps/luna-graphical-session/luna-graphical-session" <<'EOF_GRAPHICS'
#!/bin/sh
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
export XDG_SESSION_TYPE="wayland"
export XDG_SESSION_DESKTOP="niri"
export XDG_CURRENT_DESKTOP="niri"
export GTK_ICON_THEME="Adwaita"
export XCURSOR_THEME="Adwaita"
export XCURSOR_SIZE="24"
export XCURSOR_PATH="/usr/share/icons:/usr/share/cursors:/usr/share/pixmaps"
export XDG_DATA_DIRS="/usr/local/share:/usr/share"
export NOCTALIA_CONFIG_HOME="$HOME/.config"
export NOCTALIA_STATE_HOME="$HOME/.local/state"
export LIBSEAT_BACKEND="seatd"
export FONTCONFIG_FILE="/etc/fonts/fonts.conf"
export FONTCONFIG_PATH="/etc/fonts"
export LD_LIBRARY_PATH="/usr/lib/niri-compat:/usr/lib:/usr/local/lib"
export LIBGL_DRIVERS_PATH="/usr/lib/dri"
export GBM_BACKENDS_PATH="/usr/lib/gbm"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
echo "luna-graphical-session: starting user D-Bus" >&2

exec /usr/bin/dbus-run-session -- sh -c '
    dbus_ready=0
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        if /usr/bin/dbus-send --session             --dest=org.freedesktop.DBus             /org/freedesktop/DBus             org.freedesktop.DBus.ListNames >/dev/null 2>&1; then
            dbus_ready=1
            break
        fi
        sleep 0.1
    done
    if [ "$dbus_ready" -eq 1 ]; then
        echo "luna-graphical-session: user D-Bus ready" >&2
    else
        echo "luna-graphical-session: user D-Bus probe failed" >&2
    fi
    /usr/bin/dbus-send --system --dest=org.freedesktop.UPower         /org/freedesktop/UPower org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1 || true
    i=0
    while ! /usr/bin/dbus-send --system --print-reply         --dest=org.freedesktop.DBus /org/freedesktop/DBus         org.freedesktop.DBus.NameHasOwner string:org.freedesktop.UPower         2>/dev/null | grep -q "boolean true"; do
        i=$((i + 1))
        [ "$i" -ge 20 ] && break
        sleep 0.1
    done

    /usr/bin/pipewire >>"$XDG_RUNTIME_DIR/luna-pipewire.log" 2>&1 &
    PIPEWIRE_PID=$!
    /usr/bin/wireplumber >>"$XDG_RUNTIME_DIR/luna-wireplumber.log" 2>&1 &
    WIREPLUMBER_PID=$!
    echo "luna-graphical-session: PipeWire/WirePlumber started" >&2

    cleanup_children() {
        kill "$PIPEWIRE_PID" "$WIREPLUMBER_PID" 2>/dev/null || true
        wait "$PIPEWIRE_PID" "$WIREPLUMBER_PID" 2>/dev/null || true
    }
    trap cleanup_children EXIT

    echo "luna-graphical-session: starting Niri" >&2
    (
        found=0
        for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
            for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
                if [ -S "$socket" ]; then
                    echo "luna-graphical-session: Wayland display ready ($socket)" >&2
                    found=1
                    break 2
                fi
            done
            sleep 0.25
        done
        if [ "$found" -eq 0 ]; then
            echo "luna-graphical-session: Wayland display not observed" >&2
        fi
    ) &
    (
        touch "$XDG_RUNTIME_DIR/luna-noctalia.log"
        tail -f "$XDG_RUNTIME_DIR/luna-noctalia.log" >&2 &
        NOCTALIA_TAIL_PID=$!
        for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40; do
            if /usr/bin/busybox pidof noctalia >/dev/null 2>&1; then
                echo "luna-graphical-session: Noctalia process ready" >&2
                kill "$NOCTALIA_TAIL_PID" 2>/dev/null || true
                wait "$NOCTALIA_TAIL_PID" 2>/dev/null || true
                exit 0
            fi
            sleep 0.25
        done
        kill "$NOCTALIA_TAIL_PID" 2>/dev/null || true
        wait "$NOCTALIA_TAIL_PID" 2>/dev/null || true
        echo "luna-graphical-session: Noctalia process not observed" >&2
    ) &
    /usr/bin/niri --session >>"$XDG_RUNTIME_DIR/luna-niri.log" 2>&1 &
    NIRI_PID=$!
    tail -f "$XDG_RUNTIME_DIR/luna-niri.log" >&2 &
    TAIL_PID=$!
    wait "$NIRI_PID"
    NIRI_STATUS=$?
    kill "$TAIL_PID" 2>/dev/null || true
    wait "$TAIL_PID" 2>/dev/null || true
    exit "$NIRI_STATUS"
'
EOF_GRAPHICS
chmod 0755 "$SYSTEM_IMAGE_ROOT/apps/luna-graphical-session/luna-graphical-session"
install -Dm0644 "$REPO_ROOT/dist/nerd-fonts-root/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf" "$SYSTEM_IMAGE_ROOT/resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
install -Dm0644 "$REPO_ROOT/docs/design/visual-concepts/luna.png" "$SYSTEM_IMAGE_ROOT/resources/images/luna.png"

cat > "$SYSTEM_IMAGE_ROOT/config/desktop.toml" <<EOF_DESKTOP
[desktop]
compositor = "niri"
shell = "noctalia"
terminal = "ghostty"

[session]
entry = "/usr/bin/luna-user-session --handoff"
EOF_DESKTOP

cat > "$SYSTEM_IMAGE_ROOT/config/luna/niri-config.kdl" <<'EOF_NIRI_CONFIG'
input {
    keyboard {
        xkb {
            layout "us"
        }
    }
}
layout {
    gaps 8
    center-focused-column "never"
}
binds {
    Mod+Q { close-window; }
    Mod+Shift+E repeat=false { quit skip-confirmation=true; }
}
prefer-no-csd
EOF_NIRI_CONFIG

cat > "$SYSTEM_IMAGE_ROOT/config/passwd" <<'EOF_PASSWD'
root:x:0:0:root:/root:/bin/sh
messagebus:x:995:995:System Message Bus:/nonexistent:/usr/sbin/nologin
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
EOF_PASSWD

cat > "$SYSTEM_IMAGE_ROOT/config/group" <<'EOF_GROUP'
root:x:0:
shadow:x:42:
messagebus:x:995:
seat:x:997:luna
input:x:998:luna
luna:x:1000:
EOF_GROUP

cat > "$SYSTEM_IMAGE_ROOT/config/shadow" <<'EOF_SHADOW'
root:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
EOF_SHADOW
chmod 0644 "$SYSTEM_IMAGE_ROOT/config/passwd" "$SYSTEM_IMAGE_ROOT/config/group"
chmod 0600 "$SYSTEM_IMAGE_ROOT/config/shadow"

# Local interactive foundation images use the repository's existing disposable
# Alpine fixture credential (luna/luna). Keep only the Luna user's real hash in
# the bootstrap seed; root remains locked.
LUNA_TEST_HASH="$(awk -F: '$1 == "luna" {print $2; exit}' "$SYS_ROOT/etc/shadow")"
[ -n "$LUNA_TEST_HASH" ] || { echo "missing Alpine test Luna password hash" >&2; exit 1; }
printf 'root:!*:0:0:99999:7:::
luna:%s:0:0:99999:7:::
' "$LUNA_TEST_HASH" > "$SYSTEM_IMAGE_ROOT/config/shadow"
chmod 0600 "$SYSTEM_IMAGE_ROOT/config/shadow"

cat > "$SYSTEM_IMAGE_ROOT/config/nsswitch.conf" <<'EOF_NSS'
passwd: files
group: files
shadow: files
gshadow: files
hosts: files dns
services: files
networks: files
protocols: files
EOF_NSS

mksquashfs "$SYSTEM_IMAGE_ROOT" "$WORK/luna-$VERSION.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null
cp "$WORK/luna-$VERSION.squashfs" "$SYS_ROOT/images/luna-$VERSION.squashfs"
cat > "$SYS_ROOT/images/luna-$VERSION.toml" <<EOF_IMAGE_MANIFEST
[image]
name = "luna"
version = "$VERSION"
format = "squashfs"

[architecture]
arch = "x86_64"

[init]
compatible = ["$INIT_VERSION"]
EOF_IMAGE_MANIFEST

rm -rf "$RECOVERY_DATA_ROOT"
mkdir -p "$RECOVERY_DATA_ROOT/system"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$RECOVERY_DATA_ROOT/users"
mkdir -p "$RECOVERY_DATA_ROOT/cach"
mkdir -p "$RECOVERY_DATA_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
mkdir -p "$RECOVERY_DATA_ROOT/system/config/auth"
mkdir -p "$RECOVERY_DATA_ROOT/system/apps"/{recovery-tools,data-discovery,system-diagnostics,system-repair}
mkdir -p "$RECOVERY_DATA_ROOT/users/recovery"/{home,data,config}

cat > "$RECOVERY_DATA_ROOT/system/apps/recovery-tools/recovery-tools" <<'EOF_RECOVERY_TOOLS'
#!/bin/sh
printf '%s\n' 'Project Luna Recovery Tools'
printf '%s\n' 'data-discovery system-diagnostics system-repair'
EOF_RECOVERY_TOOLS

cat > "$RECOVERY_DATA_ROOT/system/apps/data-discovery/data-discovery" <<'EOF_DATA_DISCOVERY'
#!/bin/sh
printf '%s\n' 'LUNA-DATA candidates:'
for path in /dev/disk/by-partuuid/*; do
    [ -e "$path" ] || continue
    printf '%s\n' "$path"
done
EOF_DATA_DISCOVERY

cat > "$RECOVERY_DATA_ROOT/system/apps/system-diagnostics/system-diagnostics" <<'EOF_DIAGNOSTICS'
#!/bin/sh
printf '%s\n' 'Project Luna system diagnostics'
printf 'kernel: '; uname -r 2>/dev/null || printf '%s\n' unavailable
printf '%s\n' 'recovery DATA: VirtualData'
EOF_DIAGNOSTICS

cat > "$RECOVERY_DATA_ROOT/system/apps/system-repair/system-repair" <<'EOF_REPAIR'
#!/bin/sh
printf '%s\n' 'Project Luna system repair environment'
printf '%s\n' 'Physical LUNA-DATA remains a separate repair target.'
EOF_REPAIR
chmod 0755 "$RECOVERY_DATA_ROOT"/system/apps/*/*

cat > "$RECOVERY_DATA_ROOT/system/config/recovery.toml" <<EOF_RECOVERY_STATE
[recovery]
version = "$VERSION"
provider = "VirtualData"
EOF_RECOVERY_STATE

cat > "$RECOVERY_DATA_ROOT/system/config/auth/passwd" <<'EOF_REC_PASSWD'
root:x:0:0:root:/root:/bin/sh
recovery:x:1001:1001:Luna Recovery:/home/recovery:/bin/sh
EOF_REC_PASSWD
cat > "$RECOVERY_DATA_ROOT/system/config/auth/group" <<'EOF_REC_GROUP'
root:x:0:
shadow:x:42:
seat:x:997:recovery
recovery:x:1001:
EOF_REC_GROUP
cat > "$RECOVERY_DATA_ROOT/system/config/auth/shadow" <<'EOF_REC_SHADOW'
root:!*:0:0:99999:7:::
recovery:!*:0:0:99999:7:::
EOF_REC_SHADOW
chmod 0644 "$RECOVERY_DATA_ROOT/system/config/auth/passwd" "$RECOVERY_DATA_ROOT/system/config/auth/group"
chmod 0600 "$RECOVERY_DATA_ROOT/system/config/auth/shadow"

mksquashfs "$RECOVERY_DATA_ROOT" "$WORK/recovery.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null
cp "$WORK/recovery.squashfs" "$SYS_ROOT/recovery/recovery.squashfs"
cat > "$SYS_ROOT/recovery/recovery.toml" <<EOF_RECOVERY_MANIFEST
[image]
name = "recovery-data"
version = "$VERSION"
format = "squashfs"
role = "recovery"

[architecture]
arch = "x86_64"

[init]
compatible = ["$INIT_VERSION"]
EOF_RECOVERY_MANIFEST

EFI_SIZE=128
SYS_SIZE=1024
DATA_SIZE=1024
SWAP_SIZE=256
START=2048
EFI_SECTORS=$((EFI_SIZE * 2048))
SYS_START=$((START + EFI_SECTORS))
SYS_SECTORS=$((SYS_SIZE * 2048))
DATA_START=$((SYS_START + SYS_SECTORS))
DATA_SECTORS=$((DATA_SIZE * 2048))
SWAP_START=$((DATA_START + DATA_SECTORS))
SWAP_SECTORS=$((SWAP_SIZE * 2048))
END=$((SWAP_START + SWAP_SECTORS - 1))
IMAGE_MIB=$(((END + 34 + 2047) / 2048))
DISK_GUID="$(cat /proc/sys/kernel/random/uuid)"
SYS_GUID="$(cat /proc/sys/kernel/random/uuid)"
DATA_GUID="$(cat /proc/sys/kernel/random/uuid)"

sed -i "s/__DATA_GUID__/$DATA_GUID/" "$SYS_ROOT/etc/luna-data-partuuid"
printf '[data]
preferred_disk_guid = "%s"
preferred_partition_guid = "%s"
' "$DISK_GUID" "$DATA_GUID" > "$SYS_ROOT/config/luna-data.toml"

rm -rf "$DIST/luna-sys" "$DIST/luna-data"
cp -a "$SYS_ROOT" "$DIST/luna-sys"
cp -a "$DATA_ROOT" "$DIST/luna-data"
cp "$WORK/luna-$VERSION.squashfs" "$DIST/luna-$VERSION.squashfs"

truncate -s "$IMAGE_MIB"M "$OUT"
sgdisk --zap-all "$OUT" >/dev/null
sgdisk --disk-guid="$DISK_GUID"     -n "1:$START:$((SYS_START - 1))" -t 1:ef00 -c 1:EFI     -n "2:$SYS_START:$((DATA_START - 1))" -t 2:8300 -c 2:LUNA-SYS     -n "3:$DATA_START:$((SWAP_START - 1))" -t 3:8300 -c 3:LUNA-DATA     -n "4:$SWAP_START:$END" -t 4:8200 -c 4:SWAP "$OUT" >/dev/null
sgdisk --partition-guid=2:"$SYS_GUID" --partition-guid=3:"$DATA_GUID" "$OUT" >/dev/null

truncate -s "$EFI_SIZE"M "$PARTS/efi.fs"
mkfs.fat -F 32 -n EFI "$PARTS/efi.fs" >/dev/null
mmd -i "$PARTS/efi.fs" ::/EFI
mmd -i "$PARTS/efi.fs" ::/EFI/LUNA
mmd -i "$PARTS/efi.fs" ::/EFI/BOOT
mcopy -i "$PARTS/efi.fs" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI
mcopy -i "$PARTS/efi.fs" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

export SYS_ROOT DATA_ROOT PARTS SYS_SIZE DATA_SIZE SWAP_SIZE MKFS_BTRFS
fakeroot bash -c '
set -e
chown -R 0:0 "$SYS_ROOT"
chown -R 0:0 "$DATA_ROOT/system"
chown -R 1000:100 "$DATA_ROOT/users/luna"
truncate -s "${SYS_SIZE}M" "$PARTS/luna-sys.fs"
mkfs.ext4 -q -F -L LUNA-SYS -d "$SYS_ROOT" "$PARTS/luna-sys.fs" >/dev/null
truncate -s "${DATA_SIZE}M" "$PARTS/luna-data.fs"
"$MKFS_BTRFS" -q -f -L LUNA-DATA --rootdir "$DATA_ROOT" "$PARTS/luna-data.fs" >/dev/null
fallocate -l "${SWAP_SIZE}M" "$PARTS/swap.fs"
chmod 0600 "$PARTS/swap.fs"
mkswap -L SWAP "$PARTS/swap.fs" >/dev/null
' bash

dd if="$PARTS/efi.fs" of="$OUT" bs=512 seek="$START" conv=notrunc status=none
dd if="$PARTS/luna-sys.fs" of="$OUT" bs=512 seek="$SYS_START" conv=notrunc status=none
dd if="$PARTS/luna-data.fs" of="$OUT" bs=512 seek="$DATA_START" conv=notrunc status=none
dd if="$PARTS/swap.fs" of="$OUT" bs=512 seek="$SWAP_START" conv=notrunc status=none

echo "==> Verify canonical disk layout"
[ "$(sgdisk -i 1 "$OUT" | awk -F': ' '/Partition name:/ {print $2; exit}' | tr -d "'")" = EFI ]
[ "$(sgdisk -i 2 "$OUT" | awk -F': ' '/Partition name:/ {print $2; exit}' | tr -d "'")" = LUNA-SYS ]
[ "$(sgdisk -i 3 "$OUT" | awk -F': ' '/Partition name:/ {print $2; exit}' | tr -d "'")" = LUNA-DATA ]
[ "$(sgdisk -i 4 "$OUT" | awk -F': ' '/Partition name:/ {print $2; exit}' | tr -d "'")" = SWAP ]
[ "$(blkid -p -O $((SYS_START * 512)) -s TYPE -o value "$OUT")" = ext4 ]
[ "$(blkid -p -O $((DATA_START * 512)) -s TYPE -o value "$OUT")" = btrfs ]
[ "$(blkid -p -O $((SWAP_START * 512)) -s TYPE -o value "$OUT")" = swap ]

ln -sfn "luna-$VERSION.img" "$DIST/luna-pc.img"
sha256sum "$OUT" > "$DIST/luna-$VERSION.img.sha256"
cat > "$DIST/luna-$VERSION.img.BUILD-INFO" <<EOF_INFO
luna_disk_image=canonical
version=$VERSION
architecture=x86_64
partitions=EFI:$EFI_SIZE MiB,LUNA-SYS:$SYS_SIZE MiB,LUNA-DATA:$DATA_SIZE MiB,SWAP:$SWAP_SIZE MiB
luna_sys_filesystem=ext4
luna_data_filesystem=btrfs
swap_filesystem=swap
luna_sys_base=alpine-3.24.2-transitional
niri_provider=LUNA-DATA/system/apps/niri
user_boundary=LUNA-DATA/users/luna
system_image_source=LUNA-SYS/images/luna-$VERSION.squashfs
kernel=LUNA-SYS/kernels/7.2.4/bzImage
EOF_INFO

echo "Built canonical Luna disk image: $OUT"

