#!/usr/bin/env bash
set -euo pipefail

# User-facing launcher for the Luna kernel + Alpine userspace test.
# The rootfs preparation preserves Alpine shadow semantics for interactive login.
# Run this file directly from any working directory.
#
# The VM opens with a GTK window and the guest serial console is mirrored
# into the terminal. This keeps the graphical QEMU window available while
# making kernel/OpenRC/login output visible and interactive.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT_DIR/tests/ovmf/out-alpine-luna-kernel-direct-root"
SOURCE_ROOT="$ROOT_DIR/../../dist/sources/reference/alpine-3.24"
ALPINE_ROOTFS="$SOURCE_ROOT/release/alpine-runtime-rootfs-3.24.2-x86_64.tar.gz"
ROOT_PARTUUID="7A6D5A7A-0003-4C55-4E41-524F4F542020"
LUNA_KERNEL="${LUNA_KERNEL_PATH:-${ROOT_DIR}/../../dist/kernel/current/bzImage}"
NIRI_CACHE="${ROOT_DIR}/../../dist/.build/alpine-niri-apk-cache"
GREETER_APK_CACHE="$ROOT_DIR/../../dist/.build/alpine-greeter-apks/packages"
MESA_VIRGL_BUILD="${ROOT_DIR}/../../dist/.build/alpine-foundation-graphics/mesa/out"
MESA_VIRGL_LIB="${MESA_VIRGL_BUILD}/usr/local/lib"
MESA_VIRGL_DRI="${MESA_VIRGL_BUILD}/usr/lib/dri"
NOCTALIA_BUILD="${ROOT_DIR}/../../dist/.build/alpine-foundation-graphics/noctalia/out"
NOCTALIA_RUNTIME_APKS="${ROOT_DIR}/../../dist/.build/alpine-foundation-graphics/noctalia-runtime-apks"
LUNA_FONT_ROOT="${ROOT_DIR}/../../dist/nerd-fonts-root/usr/share/fonts/truetype/luna"
LUNA_LOGIN_FONT="${LUNA_FONT_ROOT}/JetBrainsMonoNerdFontMono-Regular.ttf"
LUNA_KERNEL_BUILD="${ROOT_DIR}/../../dist/kernel/linux-7.2.4/build"
INSTALL_NIRI="${LUNA_ALPINE_NIRI:-1}"
INSTALL_NATIVE_LOGIN="${LUNA_ALPINE_NATIVE_LOGIN:-1}"
if [ -n "${LUNA_ALPINE_GREETER:-}" ]; then
    INSTALL_GREETER="$LUNA_ALPINE_GREETER"
else
    INSTALL_GREETER="0"
fi
if [[ "$INSTALL_GREETER" == "1" ]]; then
    INSTALL_NATIVE_LOGIN="0"
fi
if [[ "$INSTALL_NATIVE_LOGIN" == "1" && "$INSTALL_NIRI" != "1" ]]; then
    echo "LUNA_ALPINE_NATIVE_LOGIN=1 requires LUNA_ALPINE_NIRI=1" >&2
    exit 1
fi
if [[ "$INSTALL_GREETER" == "1" && "$INSTALL_NIRI" != "1" ]]; then
    echo "LUNA_ALPINE_GREETER=1 requires LUNA_ALPINE_NIRI=1" >&2
    exit 1
fi
INTERACTIVE_SERIAL_SOCKET="${LUNA_ALPINE_INTERACTIVE_SERIAL_SOCKET:-/tmp/luna-alpine-interactive-serial.sock}"

: "${OVMF_CODE:=/usr/share/OVMF/OVMF_CODE_4M.fd}"
: "${OVMF_VARS:=/usr/share/OVMF/OVMF_VARS_4M.fd}"


# Prefer the current graphical Linux desktop session when Wayland is used.
if [[ -z "${DISPLAY:-}" && -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/wayland-0" ]]; then
    export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
    export GDK_BACKEND="${GDK_BACKEND:-wayland}"
fi

for tool in cargo qemu-system-x86_64 sgdisk mkfs.ext4 mkfs.fat mcopy mmd truncate dd tar; do
    command -v "$tool" >/dev/null || {
        echo "missing required tool: $tool" >&2
        exit 1
    }
done

for file in "$LUNA_KERNEL" "$ALPINE_ROOTFS" "$OVMF_CODE" "$OVMF_VARS"; do
    [[ -s "$file" ]] || {
        echo "missing artifact: $file" >&2
        exit 1
    }
done

rm -rf "$OUT"
mkdir -p "$OUT/system-root/alpine" "$OUT/alpine-root" "$OUT/esp"
tar -xzf "$ALPINE_ROOTFS" -C "$OUT/alpine-root"

# Native Luna SessionUI consumes the canonical Luna font resource. The Alpine
# experiment mirrors it into both the canonical resource path and an FHS path
# used by compatibility tooling; this does not change the System Image contract.
if [[ -s "$LUNA_LOGIN_FONT" ]]; then
    mkdir -p "$OUT/alpine-root/resources/fonts" "$OUT/alpine-root/usr/share/fonts/truetype/luna"
    install -Dm0644 "$LUNA_LOGIN_FONT" "$OUT/alpine-root/resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
    install -Dm0644 "$LUNA_LOGIN_FONT" "$OUT/alpine-root/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf"
else
    echo "missing Luna login font resource: $LUNA_LOGIN_FONT" >&2
    exit 1
fi

LUNA_LOGIN_ART="$ROOT_DIR/../../pic/luna.png"
if [[ -s "$LUNA_LOGIN_ART" ]]; then
    mkdir -p "$OUT/alpine-root/resources/images" "$OUT/alpine-root/usr/share/luna"
    install -Dm0644 "$LUNA_LOGIN_ART" "$OUT/alpine-root/resources/images/luna.png"
    install -Dm0644 "$LUNA_LOGIN_ART" "$OUT/alpine-root/usr/share/luna/luna-login.png"
else
    echo "missing Luna login artwork: $LUNA_LOGIN_ART" >&2
    exit 1
fi

# The release rootfs is intentionally a base userspace and does not enable
# OpenRC services. Mirror the base runlevel setup performed by Alpine's
# alpine-make-vm-image so the direct-root experiment exercises a normal
# BusyBox -> OpenRC -> getty path as well as the kernel/root handoff.
rc_add() {
    local runlevel="$1"
    shift
    local svc
    for svc in "$@"; do
        mkdir -p "$OUT/alpine-root/etc/runlevels/$runlevel"
        ln -sfn "/etc/init.d/$svc" "$OUT/alpine-root/etc/runlevels/$runlevel/$svc"
    done
}

rc_add sysinit devfs dmesg hwdrivers

cat > "$OUT/alpine-root/etc/init.d/mdev" <<'EOF'
#!/sbin/openrc-run
description="the mdev device manager"

depend() {
    provide dev
    need sysfs dev-mount
    before checkfs fsck
    keyword -containers -vserver -lxc
}

start() {
    einfo "Starting busybox mdev"
    for i in $(find /sys/devices -name 'usb[0-9]*'); do
        [ -e "$i/uevent" ] && echo add > "$i/uevent"
    done
    mdev -s
}
EOF
chmod 755 "$OUT/alpine-root/etc/init.d/mdev"
rc_add sysinit mdev
[ -e "$OUT/alpine-root/etc/init.d/cgroups" ] && rc_add sysinit cgroups || true
rc_add boot modules hwclock swap hostname sysctl bootmisc syslog
if [ -e "$OUT/alpine-root/etc/init.d/seedrng" ]; then
    rc_add boot seedrng
else
    rc_add boot urandom
fi
rc_add shutdown killprocs savecache mount-ro

# Keep a serial getty enabled in the experiment so the headless transcript
# proves that BusyBox init reached the normal login stage; tty1 remains the
# visible GTK console.
sed -i 's/^#ttyS0::respawn:/ttyS0::respawn:/' "$OUT/alpine-root/etc/inittab"
if [[ "$INSTALL_GREETER" == "1" || "$INSTALL_NATIVE_LOGIN" == "1" ]]; then
    sed -i 's/^tty1::respawn:/#tty1::respawn:/' "$OUT/alpine-root/etc/inittab"
fi
bash "$ROOT_DIR/tests/ovmf/configure-alpine-test-user.sh" "$OUT/alpine-root"
if [[ "${LUNA_ALPINE_TEST_ROOT_PASSWORDLESS:-0}" == "1" ]]; then
    # Test-only convenience: permit the disposable root account to enter the
    # serial console without a password so session lifecycle can be exercised.
    sed -i 's/^root:[^:]*:/root::/' "$OUT/alpine-root/etc/shadow"
fi
mkdir -p "$OUT/alpine-root/lib/modules"

if [[ "$INSTALL_NIRI" == "1" ]]; then
    echo "==> Installing native Alpine niri runtime from project cache..."
    [[ -d "$NIRI_CACHE" ]] || { echo "missing niri cache: $NIRI_CACHE" >&2; exit 1; }
    [[ -s "$NIRI_CACHE/niri-25.11-r2.apk" ]] || { echo "missing niri APK" >&2; exit 1; }
    for apk in "$NIRI_CACHE"/*.apk; do
        case "$(basename "$apk")" in
            eudev-libs-*.apk) continue ;;
        esac
        tar -xzf "$apk" -C "$OUT/alpine-root" \
            --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' \
            2>/dev/null
    done

    # Use the Alpine-provided OpenRC seatd service verbatim from aports.
    # Add a second serial getty used by the host-side test harness. ttyS0 stays
    # the kernel console/transcript; ttyS1 is an independent interactive login
    # channel so the visible GTK window remains available for graphics tests.
    if ! grep -q '^ttyS1::respawn:' "$OUT/alpine-root/etc/inittab"; then
        printf '%s\n' 'ttyS1::respawn:/sbin/getty -L 115200 ttyS1 vt100' >> "$OUT/alpine-root/etc/inittab"
    fi

    mkdir -p "$OUT/alpine-root/etc/init.d" "$OUT/alpine-root/etc/conf.d"
    cp "$SOURCE_ROOT/aports-3.24/community/seatd/seatd.initd" "$OUT/alpine-root/etc/init.d/seatd"
    cp "$SOURCE_ROOT/aports-3.24/community/seatd/seatd.confd" "$OUT/alpine-root/etc/conf.d/seatd"
    chmod 755 "$OUT/alpine-root/etc/init.d/seatd"

    # seatd uses a dedicated group for its socket; every graphical test user
    # needs access so libseat can acquire the DRM seat after credential drop.
    seat_users="luna"
    if [[ "${LUNA_ALPINE_TEST_MULTIUSER:-0}" == "1" ]]; then
        seat_users="luna,alice,bob"
    fi
    if ! grep -q '^seat:' "$OUT/alpine-root/etc/group"; then
        printf '%s\n' "seat:x:83:${seat_users}" >> "$OUT/alpine-root/etc/group"
    else
        sed -i '/^seat:/d' "$OUT/alpine-root/etc/group"
        printf '%s\n' "seat:x:83:${seat_users}" >> "$OUT/alpine-root/etc/group"
    fi
    mkdir -p "$OUT/alpine-root/etc/runlevels/boot"
    ln -sfn /etc/init.d/seatd "$OUT/alpine-root/etc/runlevels/boot/seatd"


    if [[ "$INSTALL_GREETER" == "1" || "$INSTALL_NATIVE_LOGIN" == "1" ]]; then
        if [[ "$INSTALL_GREETER" == "1" ]]; then
            echo "==> Installing optional greetd + gtkgreet + Cage fallback from Alpine v3.24 cache..."
        test -d "$GREETER_APK_CACHE" || { echo "missing greeter APK cache: $GREETER_APK_CACHE" >&2; exit 1; }
        shopt -s nullglob
        greeter_apks=("$GREETER_APK_CACHE"/*.apk)
        ((${#greeter_apks[@]} > 0)) || { echo "greeter APK cache is empty: $GREETER_APK_CACHE" >&2; exit 1; }
        for apk in "${greeter_apks[@]}"; do
            tar -xzf "$apk" -C "$OUT/alpine-root" --exclude=".PKGINFO" --exclude=".SIGN.*" --exclude=".INSTALL" --exclude=".trigger" 2>/dev/null
        done
        shopt -u nullglob
    fi

        service_apks=(
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
            acl-libs-2.3.2-r1.apk
            duktape-libs-2.7.0-r3.apk
            polkit-127-r2.apk
            polkit-common-127-r2.apk
            polkit-elogind-127-r2.apk
            polkit-elogind-libs-127-r2.apk
            polkit-openrc-127-r2.apk
            rtkit-0.14-r0.apk
            rtkit-openrc-0.14-r0.apk
            accountsservice-23.13.9-r3.apk
        )
        for apk_name in "${service_apks[@]}"; do
            apk="$GREETER_APK_CACHE/$apk_name"
            test -s "$apk" || { echo "missing graphical service APK: $apk_name" >&2; exit 1; }
            tar -xzf "$apk" -C "$OUT/alpine-root" --exclude=".PKGINFO" --exclude=".SIGN.*" --exclude=".INSTALL" --exclude=".trigger" 2>/dev/null
        done
        cat > "$OUT/alpine-root/etc/init.d/dbus" <<'EOF'
#!/sbin/openrc-run
name="System Message Bus"
description="D-Bus System Message Bus"

extra_started_commands="reload"

command="/usr/bin/dbus-daemon"
command_args="--system --nofork --nopidfile --syslog-only ${command_args:-}"
command_background="yes"
pidfile="/run/$RC_SVCNAME.pid"

start_pre() {
    checkpath -d -m755 -o root:messagebus /run/dbus || return 1
    if [ -x /usr/libexec/dbus-daemon-launch-helper ]; then
        chown root:messagebus /usr/libexec/dbus-daemon-launch-helper
        chmod 4750 /usr/libexec/dbus-daemon-launch-helper
    fi
    chown -R root:root /usr/share/dbus-1/system-services /usr/share/dbus-1/system.d 2>/dev/null || true
    /usr/bin/dbus-uuidgen --ensure=${machine_id:-/etc/machine-id}
}

depend() {
    need localmount
    after bootmisc
}

stop_post() {
    [ ! -S /run/dbus/system_bus_socket ] || rm -f /run/dbus/system_bus_socket
}

reload() {
    ebeg "Reloading $name configuration"
    /usr/bin/dbus-send --print-reply --system --type=method_call         --dest=org.freedesktop.DBus / org.freedesktop.DBus.ReloadConfig >/dev/null
    eend $?
}
EOF
        chmod 755 "$OUT/alpine-root/etc/init.d/dbus"

        cat > "$OUT/alpine-root/etc/init.d/upower" <<'EOF'
#!/sbin/openrc-run
name="UPower"
description="Power management daemon"

command="/usr/libexec/upowerd"
command_background="yes"
pidfile="/run/upowerd.pid"

depend() {
    need dbus
    after elogind
}
EOF
        chmod 755 "$OUT/alpine-root/etc/init.d/upower"

        grep -q "^greetd:" "$OUT/alpine-root/etc/group" || echo "greetd:x:996:" >> "$OUT/alpine-root/etc/group"
        grep -q "^greetd:" "$OUT/alpine-root/etc/passwd" || echo "greetd:x:996:83:greetd:/var/lib/greetd:/sbin/nologin" >> "$OUT/alpine-root/etc/passwd"
        grep -q "^messagebus:" "$OUT/alpine-root/etc/group" || echo "messagebus:x:997:" >> "$OUT/alpine-root/etc/group"
        grep -q "^messagebus:" "$OUT/alpine-root/etc/passwd" || echo "messagebus:x:997:997:messagebus:/dev/null:/sbin/nologin" >> "$OUT/alpine-root/etc/passwd"
        grep -q "^polkitd:" "$OUT/alpine-root/etc/group" || echo "polkitd:x:998:" >> "$OUT/alpine-root/etc/group"
        grep -q "^polkitd:" "$OUT/alpine-root/etc/passwd" || echo "polkitd:x:998:998:polkitd:/var/empty:/sbin/nologin" >> "$OUT/alpine-root/etc/passwd"
        grep -q "^rtkit:" "$OUT/alpine-root/etc/group" || echo "rtkit:x:995:" >> "$OUT/alpine-root/etc/group"
        grep -q "^rtkit:" "$OUT/alpine-root/etc/passwd" || echo "rtkit:x:995:995:rtkit:/dev/null:/sbin/nologin" >> "$OUT/alpine-root/etc/passwd"
        grep -q "^plugdev:" "$OUT/alpine-root/etc/group" || echo "plugdev:x:46:" >> "$OUT/alpine-root/etc/group"
        grep -q "^seat:" "$OUT/alpine-root/etc/group" || echo "seat:x:83:luna,greetd" >> "$OUT/alpine-root/etc/group"
        awk -F: 'BEGIN { OFS=":" } $1=="video" { if ($4=="") $4="greetd"; else if ($4 !~ /(^|,)greetd(,|$)/) $4=$4",greetd" } { print }' "$OUT/alpine-root/etc/group" > "$OUT/alpine-root/etc/group.tmp"
        mv "$OUT/alpine-root/etc/group.tmp" "$OUT/alpine-root/etc/group"
        awk -F: 'BEGIN { OFS=":" } $1=="input" { if ($4=="") $4="greetd"; else if ($4 !~ /(^|,)greetd(,|$)/) $4=$4",greetd" } { print }' "$OUT/alpine-root/etc/group" > "$OUT/alpine-root/etc/group.tmp"
        mv "$OUT/alpine-root/etc/group.tmp" "$OUT/alpine-root/etc/group"

        mkdir -p "$OUT/alpine-root/etc/pam.d"
        cat > "$OUT/alpine-root/etc/pam.d/greetd" <<'EOF'
# Disposable Alpine/Luna QEMU greeter configuration.
auth required pam_unix.so
account required pam_permit.so
session optional pam_gnome_keyring.so auto_start
session required pam_elogind.so
session required pam_permit.so
EOF

        mkdir -p "$OUT/alpine-root/etc/greetd" "$OUT/alpine-root/var/lib/greetd"
        chmod 750 "$OUT/alpine-root/var/lib/greetd"

        # greetd's session worker does not inherit the seat group reliably in
        # this hand-built rootfs. Keep the seat socket private, but make it
        # accessible to the dedicated greetd group before the greeter starts.
        cat > "$OUT/alpine-root/etc/init.d/greetd" <<'EOF'
#!/sbin/openrc-run
name="greetd"
description="Greeter daemon"
cfgfile="/etc/greetd/config.toml"
command="/usr/sbin/greetd"
command_args="--config $cfgfile"
command_background="yes"
pidfile="/run/greetd.pid"
required_files="$cfgfile"

start_pre() {
    local homedir
    homedir="$(getent passwd greetd | cut -d: -f6)"
    checkpath -d -m750 -o greetd:greetd "$homedir"
    checkpath -d -m700 -o greetd:greetd /run/greetd-runtime
    if [ -S /run/seatd.sock ]; then
        # seatd itself uses the dedicated `seat` group. Both the greetd
        # greeter and the logged-in Luna user are members of that group.
        chgrp seat /run/seatd.sock
        chmod 660 /run/seatd.sock
    fi
    if command -v fc-cache >/dev/null 2>&1; then
        fc-cache -f >/dev/null 2>&1 || true
    fi
    if command -v gdk-pixbuf-query-loaders >/dev/null 2>&1; then
        gdk-pixbuf-query-loaders --update-cache
    fi
    if command -v update-mime-database >/dev/null 2>&1 && [ -d /usr/share/mime ]; then
        update-mime-database /usr/share/mime >/dev/null 2>&1 || true
    fi
}

depend() {
    need localmount dbus elogind seatd
    provide display-manager
}
EOF

        mkdir -p "$OUT/alpine-root/usr/share/icons/default"
        cat > "$OUT/alpine-root/usr/share/icons/default/index.theme" <<'EOF'
[Icon Theme]
Name=Default
Comment=Default cursor theme for the Alpine/Luna test root
Inherits=Adwaita
EOF

        cat > "$OUT/alpine-root/etc/greetd/config.toml" <<'EOF'
[terminal]
vt = 1

[default_session]
command = "/usr/local/bin/luna-greetd-session"
user = "greetd"
EOF

        cat > "$OUT/alpine-root/usr/local/bin/luna-greetd-session" <<'EOF'
#!/bin/sh
export XDG_RUNTIME_DIR="/run/greetd-runtime"
export LIBSEAT_BACKEND="seatd"
export GTK_THEME="Adwaita:dark"
export GTK_ICON_THEME="Adwaita"
export XCURSOR_THEME="Adwaita"
export XCURSOR_SIZE="24"
export XCURSOR_PATH="/usr/share/icons"
export XDG_DATA_DIRS="/usr/local/share:/usr/share"
export LD_LIBRARY_PATH="/opt/luna-test/mesa-virgl/lib"
export LIBGL_DRIVERS_PATH="/opt/luna-test/mesa-virgl/dri"
export GBM_BACKENDS_PATH="/opt/luna-test/mesa-virgl/lib/gbm"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
exec cage -s -- gtkgreet -c /usr/local/bin/luna-graphical-session >>/run/greetd-runtime/luna-greeter.log 2>&1
EOF
        chmod 755 "$OUT/alpine-root/usr/local/bin/luna-greetd-session"

        mkdir -p "$OUT/alpine-root/etc/local.d" "$OUT/alpine-root/var/log"
        cat > "$OUT/alpine-root/usr/local/bin/luna-graphical-session" <<'EOF'
#!/bin/sh
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
export XDG_SESSION_TYPE="wayland"
export XDG_SESSION_DESKTOP="niri"
export XDG_CURRENT_DESKTOP="niri"
export GTK_ICON_THEME="Adwaita"
export XCURSOR_THEME="Adwaita"
export XCURSOR_SIZE="24"
export XCURSOR_PATH="/usr/share/icons"
export XDG_DATA_DIRS="/usr/local/share:/usr/share"
export NOCTALIA_CONFIG_HOME="$HOME/.config"
export NOCTALIA_STATE_HOME="$HOME/.local/state"
export LIBSEAT_BACKEND="seatd"
export LD_LIBRARY_PATH="/opt/luna-test/mesa-virgl/lib"
export LIBGL_DRIVERS_PATH="/opt/luna-test/mesa-virgl/dri"
export GBM_BACKENDS_PATH="/opt/luna-test/mesa-virgl/lib/gbm"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
exec dbus-run-session -- sh -c '
    # Warm up UPower through the real system D-Bus before WirePlumber loads
    # its BlueZ SPA plugin. This avoids the startup race where the plugin
    # queries UPower before its D-Bus service has been activated.
    /usr/bin/dbus-send --system --dest=org.freedesktop.UPower         /org/freedesktop/UPower org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1 || true
    i=0
    while ! /usr/bin/dbus-send --system --print-reply --dest=org.freedesktop.DBus         /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner         string:org.freedesktop.UPower 2>/dev/null | grep -q "boolean true"; do
        i=$((i + 1))
        [ "$i" -ge 20 ] && break
        sleep 0.1
    done
    /usr/bin/pipewire >>"$XDG_RUNTIME_DIR/luna-pipewire.log" 2>&1 &
    PIPEWIRE_PID=$!
    /usr/bin/wireplumber >>"$XDG_RUNTIME_DIR/luna-wireplumber.log" 2>&1 &
    WIREPLUMBER_PID=$!

    cleanup_children() {
        kill "$PIPEWIRE_PID" "$WIREPLUMBER_PID" 2>/dev/null || true
        wait "$PIPEWIRE_PID" "$WIREPLUMBER_PID" 2>/dev/null || true
    }

    snapshot_session_logs() {
        LOG_DIR="$HOME/.local/state/noctalia/logs"
        mkdir -p "$LOG_DIR" 2>/dev/null || true
        for log in luna-niri.log luna-pipewire.log luna-wireplumber.log luna-noctalia.log; do
            if [ -f "$XDG_RUNTIME_DIR/$log" ]; then
                cp "$XDG_RUNTIME_DIR/$log" "$LOG_DIR/$log" 2>/dev/null || true
            fi
        done
    }

    trap cleanup_children EXIT

    /usr/bin/niri --session >>"$XDG_RUNTIME_DIR/luna-niri.log" 2>&1
    NIRI_STATUS=$?
    snapshot_session_logs
    exit "$NIRI_STATUS"
'
EOF
        chmod 755 "$OUT/alpine-root/usr/local/bin/luna-graphical-session"

        cat > "$OUT/alpine-root/usr/local/bin/luna-noctalia-test" <<'EOF'
#!/bin/sh
export NOCTALIA_LOG_LEVEL=debug
exec /usr/local/bin/noctalia >>"${XDG_RUNTIME_DIR:-/tmp}/luna-noctalia.log" 2>&1
EOF
        chmod 755 "$OUT/alpine-root/usr/local/bin/luna-noctalia-test"

        cat > "$OUT/alpine-root/usr/local/bin/luna-noctalia-power-test" <<'EOF'
#!/bin/sh
sleep 8
ACTION="${1:-reboot}"
LOG="$HOME/.local/state/noctalia/power-test.log"
printf 'testing Noctalia session action=%s\n' "$ACTION" > "$LOG"
/usr/local/bin/noctalia msg session "$ACTION" >> "$LOG" 2>&1
printf 'Noctalia session action=%s command returned\n' "$ACTION" >> "$LOG"
EOF
        chmod 755 "$OUT/alpine-root/usr/local/bin/luna-noctalia-power-test"

        if [[ "$INSTALL_NATIVE_LOGIN" == "1" ]]; then
            echo "==> Building native Luna UserSession..."
            cargo build --release -p luna-user-session --target x86_64-unknown-linux-musl
            SESSION_BINARY="$ROOT_DIR/../../target/x86_64-unknown-linux-musl/release/luna-user-session"
            test -x "$SESSION_BINARY" || { echo "missing native UserSession binary: $SESSION_BINARY" >&2; exit 1; }
            install -Dm0755 "$SESSION_BINARY" "$OUT/alpine-root/usr/bin/luna-user-session"

            cat > "$OUT/alpine-root/usr/local/bin/luna-native-session" <<'EOF'
#!/bin/sh
echo "Luna UserSession: native login process starting" >/dev/ttyS0
exec /usr/bin/luna-user-session --login-launch /usr/local/bin/luna-graphical-session >>/dev/ttyS0 2>&1
EOF
            chmod 755 "$OUT/alpine-root/usr/local/bin/luna-native-session"

            cat > "$OUT/alpine-root/etc/init.d/luna-session" <<'EOF'
#!/sbin/openrc-run
name="Luna UserSession"
description="Native Luna graphical login and user session"
command="/usr/local/bin/luna-native-session"
command_args=""
command_background="yes"
pidfile="/run/luna-session.pid"
respawn_delay=2
respawn_max=0

depend() {
    need localmount dbus elogind seatd
    provide display-manager
}
EOF
            chmod 755 "$OUT/alpine-root/etc/init.d/luna-session"
        fi

        mkdir -p "$OUT/alpine-root/etc/runlevels/default"
        services="cgroups dbus elogind upower hostname bluetooth networkmanager polkit rtkit"
        if [[ "$INSTALL_GREETER" == "1" ]]; then
            services="$services greetd"
        fi
        for svc in $services; do
            if [ -e "$OUT/alpine-root/etc/init.d/$svc" ]; then
                ln -sfn "/etc/init.d/$svc" "$OUT/alpine-root/etc/runlevels/default/$svc"
            fi
        done
        if [[ "$INSTALL_NATIVE_LOGIN" == "1" ]]; then
            ln -sfn /etc/init.d/luna-session "$OUT/alpine-root/etc/runlevels/default/luna-session"
        fi
        if [[ "$INSTALL_GREETER" == "1" ]]; then
            if ! grep -q "^rc_need=seatd$" "$OUT/alpine-root/etc/conf.d/greetd"; then
                echo "rc_need=seatd" >> "$OUT/alpine-root/etc/conf.d/greetd"
            fi
        fi
    fi

    test -s "$NIRI_CACHE/wayland-libs-server-1.25.0-r0.apk" || { echo "missing wayland-libs-server APK" >&2; exit 1; }
    test -s "$NIRI_CACHE/mesa-dri-gallium-26.1.6-r0.apk" || { echo "missing mesa-dri-gallium APK" >&2; exit 1; }
    test -s "$NIRI_CACHE/mesa-utils-9.0.0-r6.apk" || { echo "missing mesa-utils APK" >&2; exit 1; }
    test -s $MESA_VIRGL_LIB/libEGL.so.1.0.0 || { echo "missing Mesa virgl runtime: $MESA_VIRGL_BUILD" >&2; exit 1; }
    mkdir -p "$OUT/alpine-root/opt/luna-test/mesa-virgl/lib" "$OUT/alpine-root/opt/luna-test/mesa-virgl/dri"
    cp -a "$MESA_VIRGL_LIB/." "$OUT/alpine-root/opt/luna-test/mesa-virgl/lib/"
    cp -a "$MESA_VIRGL_DRI/." "$OUT/alpine-root/opt/luna-test/mesa-virgl/dri/"

    test -x "$NOCTALIA_BUILD/usr/local/bin/noctalia" || { echo "missing Noctalia build: $NOCTALIA_BUILD" >&2; exit 1; }
    test -d "$NOCTALIA_RUNTIME_APKS" || { echo "missing Noctalia runtime package set: $NOCTALIA_RUNTIME_APKS" >&2; exit 1; }
    mkdir -p "$OUT/alpine-root/usr/local"
    cp -a "$NOCTALIA_BUILD/usr/local/." "$OUT/alpine-root/usr/local/"
    for pkg in sdbus-cpp libsecret polkit-noelogind-libs wireplumber-libs libqalculate libical tomlplusplus libjxl libsndfile libwebp libwebpdemux libcurl md4c-libs librsvg libsodium wayland-libs-egl linux-pam gdk-pixbuf libdav1d libgcrypt c-ares nghttp2-libs libpsl icu-libs mpfr4 libsharpyuv libhwy libogg libvorbis libflac mpg123-libs lame-libs lcms2 libglycin libgpg-error libseccomp lua5.4-libs dbus pipewire wireplumber; do
        apk="$(find "$NOCTALIA_RUNTIME_APKS" -maxdepth 1 -name "$pkg-[0-9]*.apk" | head -1)"
        test -n "$apk" || { echo "missing Noctalia runtime APK: $pkg" >&2; exit 1; }
        tar -xzf "$apk" -C "$OUT/alpine-root" --exclude=".PKGINFO" --exclude=".SIGN.*" --exclude=".INSTALL" --exclude=".trigger" 2>/dev/null
    done

    if [[ -s "$LUNA_KERNEL_BUILD/modules.builtin" ]]; then
        mkdir -p "$OUT/alpine-root/lib/modules/7.2.4"
        cp "$LUNA_KERNEL_BUILD/modules.builtin" "$OUT/alpine-root/lib/modules/7.2.4/modules.builtin"
        if [[ -s "$LUNA_KERNEL_BUILD/modules.builtin.modinfo" ]]; then
            cp "$LUNA_KERNEL_BUILD/modules.builtin.modinfo" "$OUT/alpine-root/lib/modules/7.2.4/modules.builtin.modinfo"
        fi
        depmod -b "$OUT/alpine-root" 7.2.4 >/dev/null 2>&1 || true
    fi

    if [ -f "$OUT/alpine-root/usr/lib/sysctl.d/00-alpine.conf" ]; then
    sed -i '/^[[:space:]]*kernel\.unprivileged_bpf_disabled[[:space:]]*=/d' "$OUT/alpine-root/usr/lib/sysctl.d/00-alpine.conf"
fi

if command -v dbus-uuidgen >/dev/null 2>&1; then
        dbus-uuidgen --ensure="$OUT/alpine-root/etc/machine-id"
    fi

    # Adapt Alpine's system-bus activation files for the OpenRC test root:
    # SystemdService= is not usable here because this experiment has no systemd.
    for svc in         "$OUT/alpine-root/usr/share/dbus-1/system-services/org.freedesktop.UPower.service"         "$OUT/alpine-root/usr/share/dbus-1/system-services/org.freedesktop.Accounts.service"         "$OUT/alpine-root/usr/share/dbus-1/system-services/org.freedesktop.PolicyKit1.service"; do
        if [ -f "$svc" ]; then
            sed -i '/^SystemdService=/d' "$svc"
            chmod 644 "$svc"
        fi
    done

    mkdir -p "$OUT/alpine-root/etc/local.d"
    cat > "$OUT/alpine-root/etc/local.d/luna-runtime.start" <<'EOF'
#!/bin/sh
chown root:root /tmp
chmod 1777 /tmp
mkdir -p /tmp/.X11-unix
chown root:root /tmp/.X11-unix
chmod 1777 /tmp/.X11-unix
mkdir -p /run/user/1000
chown 1000:1000 /run/user/1000
chmod 700 /run/user/1000
mkdir -p /run/user/996
chown 996:83 /run/user/996
chmod 700 /run/user/996
# The minimal rootfs intentionally keeps libudev-zero. NetworkManager therefore
# cannot take ownership of the virtio NIC until a full udev stack is introduced.
# For this QEMU-only foundation test, use the known-good BusyBox DHCP path.
if [ -x /sbin/udhcpc ]; then
    /sbin/ip link set eth0 up >/dev/null 2>&1 || true
    /sbin/udhcpc -i eth0 -q -n -t 5 >/dev/null 2>&1 || true
fi
if [ ! -e /etc/resolv.conf ]; then
    printf 'nameserver 10.0.2.3\n' > /etc/resolv.conf
fi
EOF
    chmod 755 "$OUT/alpine-root/etc/local.d/luna-runtime.start"

    printf 'nameserver 10.0.2.3\n' > "$OUT/alpine-root/etc/resolv.conf"
    mkdir -p "$OUT/alpine-root/etc/runlevels/default"
    if [ -e "$OUT/alpine-root/etc/init.d/local" ]; then
        ln -sfn /etc/init.d/local "$OUT/alpine-root/etc/runlevels/default/local"
    fi
    mkdir -p "$OUT/alpine-root/home/luna/.config/niri"
    mkdir -p "$OUT/alpine-root/home/luna/.config/noctalia"
    cat > "$OUT/alpine-root/home/luna/.config/noctalia/00-luna-qemu.toml" <<'EOF'
# QEMU foundation profile: this virtual machine has no physical thermal sensor.
# Keep the rest of Noctalia enabled while disabling the sensor-based monitor
# service so the test desktop does not report a false hardware warning.
[system.monitor]
enabled = false
EOF
    mkdir -p "$OUT/alpine-root/home/luna/.local/state/noctalia"
    # Keep the real Noctalia first-run marker lifecycle: the wizard may appear
    # once, then its completion marker persists in the user's state directory.
    cat > "$OUT/alpine-root/home/luna/.config/noctalia/01-luna-session.toml" <<'EOF'
[shell]
# Let Noctalia manage its real first-run wizard marker in ~/.local/state.
polkit_agent = false

[shell.session.power]
reboot = "loginctl reboot"
shutdown = "loginctl poweroff"
EOF
    chown -R 1000:1000 "$OUT/alpine-root/home/luna/.config/noctalia" "$OUT/alpine-root/home/luna/.local"
    cat > "$OUT/alpine-root/.etc-luna-power-rules" <<'EOF'
polkit.addRule(function(action, subject) {
    if (!subject.isInGroup("wheel")) {
        return;
    }

    if (action.id == "org.freedesktop.login1.reboot" ||
        action.id == "org.freedesktop.login1.power-off" ||
        action.id == "org.freedesktop.login1.power-off-ignore-inhibit" ||
        action.id == "org.freedesktop.login1.reboot-ignore-inhibit") {
        return polkit.Result.YES;
    }

});
EOF
    install -Dm0644 "$OUT/alpine-root/.etc-luna-power-rules" "$OUT/alpine-root/etc/polkit-1/rules.d/49-luna-qemu-power.rules"
    rm -f "$OUT/alpine-root/.etc-luna-power-rules"
    cat > "$OUT/alpine-root/home/luna/.config/niri/config.kdl" <<'EOF'
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
spawn-at-startup "/usr/local/bin/luna-noctalia-test"
EOF
    chown -R 1000:1000 "$OUT/alpine-root/home/luna/.config"
    if [[ "${LUNA_ALPINE_TEST_MULTIUSER:-0}" == "1" ]]; then
        for test_user in alice bob; do
            mkdir -p "$OUT/alpine-root/home/$test_user/.config/niri"
            mkdir -p "$OUT/alpine-root/home/$test_user/.config/noctalia"
            mkdir -p "$OUT/alpine-root/home/$test_user/.local/state/noctalia"
            cp "$OUT/alpine-root/home/luna/.config/niri/config.kdl" "$OUT/alpine-root/home/$test_user/.config/niri/config.kdl"
            cp "$OUT/alpine-root/home/luna/.config/noctalia/01-luna-session.toml" "$OUT/alpine-root/home/$test_user/.config/noctalia/01-luna-session.toml"
            cp "$OUT/alpine-root/home/luna/.config/noctalia/00-luna-qemu.toml" "$OUT/alpine-root/home/$test_user/.config/noctalia/00-luna-qemu.toml"
            chmod 755 "$OUT/alpine-root/home/$test_user/.config" "$OUT/alpine-root/home/$test_user/.config/niri"
            chmod 644 "$OUT/alpine-root/home/$test_user/.config/niri/config.kdl"
        done
    fi
fi
echo "==> Using Luna kernel: $LUNA_KERNEL"
echo "==> Building luna-boot..."
cp "$LUNA_KERNEL" "$OUT/system-root/alpine/luna-kernel"

FEATURES=alpine-direct-root
if [[ -n "${LUNA_ALPINE_DEBUG:-}" ]]; then
    FEATURES="alpine-direct-root,alpine-debug"
fi

cargo build --release \
    --manifest-path "$ROOT_DIR/Cargo.toml" \
    --features "$FEATURES" \
    --target x86_64-unknown-uefi

EFI="$ROOT_DIR/target/x86_64-unknown-uefi/release/luna-boot.efi"

echo "==> Creating EFI system partition..."
truncate -s 64M "$OUT/esp.img"
mkfs.fat -F 32 "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI
mmd -i "$OUT/esp.img" ::/EFI/BOOT
mmd -i "$OUT/esp.img" ::/EFI/LUNA
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI
mcopy -i "$OUT/esp.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

echo "==> Creating LUNA-SYS..."
mkfs.ext4 -q -F -L LUNA-SYS "$OUT/system.img" 256M
debugfs -w -R "mkdir /alpine" "$OUT/system.img" >/dev/null 2>&1
debugfs -w -R "write $LUNA_KERNEL /alpine/luna-kernel" "$OUT/system.img" >/dev/null 2>&1
mkfs.ext4 -q -F -L ALPINE-ROOT -d "$OUT/alpine-root" "$OUT/alpine-root.img" 768M
# APK extraction into the host staging tree cannot preserve guest root ownership
# or setuid bits. Restore the security-sensitive PolicyKit metadata in the image.
debugfs -w -R "set_inode_field /usr/bin/pkexec uid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/bin/pkexec gid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/bin/pkexec mode 0104755" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkit-agent-helper-1 uid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkit-agent-helper-1 gid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkit-agent-helper-1 mode 0104755" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkitd uid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkitd gid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /usr/lib/polkit-1/polkitd mode 0100755" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1 uid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1 gid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1 mode 040755" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1/rules.d uid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1/rules.d gid 0" "$OUT/alpine-root.img" >/dev/null
debugfs -w -R "set_inode_field /etc/polkit-1/rules.d mode 040755" "$OUT/alpine-root.img" >/dev/null
if [[ "${LUNA_ALPINE_TEST_MULTIUSER:-0}" == "1" ]]; then
    owner_commands="$OUT/alpine-root-owners.debugfs"
    : > "$owner_commands"
    for test_user in alice bob; do
        test_user_id="$(awk -F: -v u="$test_user" '$1 == u { print $3 }' "$OUT/alpine-root/etc/passwd")"
        while IFS= read -r path; do
            rel="/${path#"$OUT/alpine-root"/}"
            printf 'set_inode_field %s uid %s\n' "$rel" "$test_user_id" >> "$owner_commands"
            printf 'set_inode_field %s gid 100\n' "$rel" >> "$owner_commands"
        done < <(find "$OUT/alpine-root/home/$test_user" -print)
    done
    debugfs -w -f "$owner_commands" "$OUT/alpine-root.img" >/dev/null
    rm -f "$owner_commands"
fi

echo "==> Creating GPT test disk..."
truncate -s 1400M "$OUT/disk.img"
sgdisk --zap-all "$OUT/disk.img" >/dev/null
sgdisk \
    --disk-guid=7A6D5A7A-0000-4C55-4E41-53444449534B \
    -n 1:2048:+64M -t 1:ef00 -c 1:EFI \
    -n 2:133120:+256M -t 2:8300 -c 2:LUNA-SYS \
    -n 3:657408:+768M -t 3:8300 -c 3:ALPINE-ROOT \
    "$OUT/disk.img" >/dev/null
sgdisk \
    --partition-guid=1:7A6D5A7A-0001-4C55-4E41-454649202020 \
    --partition-guid=2:7A6D5A7A-0002-4C55-4E41-53595354454D \
    --partition-guid=3:$ROOT_PARTUUID \
    "$OUT/disk.img" >/dev/null

dd if="$OUT/esp.img" of="$OUT/disk.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/system.img" of="$OUT/disk.img" bs=512 seek=133120 conv=notrunc status=none
dd if="$OUT/alpine-root.img" of="$OUT/disk.img" bs=512 seek=657408 conv=notrunc status=none
cp "$OVMF_VARS" "$OUT/OVMF_VARS.fd"

SERIAL_LOG="${LUNA_QEMU_SERIAL_LOG:-$OUT/serial.log}"
if [[ "$INSTALL_NIRI" == "1" ]]; then
    QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-sdl,gl=on}"
else
    QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-gtk}"
fi

mkdir -p "$(dirname "$SERIAL_LOG")"

echo
echo "============================================================"
echo " Luna OS / Alpine / Luna kernel / direct-root test"
echo "============================================================"
echo "QEMU display : $QEMU_DISPLAY"
echo "Serial log   : $SERIAL_LOG"
if [[ "$INSTALL_NIRI" == "1" ]]; then
    echo "Interactive ttyS1: $INTERACTIVE_SERIAL_SOCKET"
fi
echo "Root         : PARTUUID=$ROOT_PARTUUID"
echo "Disk image   : $OUT/disk.img"
echo
echo "The QEMU graphics window will open now."
echo "Guest boot/login is rendered in the QEMU graphics window."
echo "No initramfs, chroot, switch_root or pivot_root is used."
echo "Serial transcript is saved to: $SERIAL_LOG"
if [[ -n "${LUNA_QEMU_SERIAL_STDIO:-}" ]]; then
    echo "Host serial console: enabled (debug mode)"
fi
echo "============================================================"
echo

# The guest console is tty0, so normal boot/login interaction belongs in the
# QEMU graphics window. Serial output is saved to SERIAL_LOG by default.
#
# Set LUNA_QEMU_SERIAL_STDIO=1 when a live serial console in the host terminal
# is needed for debugging.
if [[ -n "${LUNA_QEMU_SERIAL_STDIO:-}" ]]; then
    SERIAL_ARGS=(-serial stdio)
else
    SERIAL_ARGS=(-serial "file:${SERIAL_LOG}")
fi

GPU_ARGS=()
if [[ "$INSTALL_NIRI" == "1" ]]; then
    # Niri requires a hardware EGL renderer. QEMU's virgl-backed virtio GPU
    # exposes the guest virgl renderer while SDL provides the visible host window
    # on this machine (the GTK backend has no OpenGL support).
    GPU_ARGS=(-vga none -device virtio-gpu-gl-pci)
    QEMU_DISPLAY_ARGS=("$QEMU_DISPLAY")
else
    QEMU_DISPLAY_ARGS=("$QEMU_DISPLAY")
fi

rm -f "$INTERACTIVE_SERIAL_SOCKET" "$OUT/monitor.sock"

qemu-system-x86_64 \
    -name "Luna OS - Alpine on Luna Kernel" \
    -machine q35 \
    -boot order=c,menu=off,strict=on \
    -m 2G -smp 4 -accel kvm -cpu host \
    -nic user,model=virtio-net-pci \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$OUT/OVMF_VARS.fd" \
    -drive format=raw,file="$OUT/disk.img" \
    -monitor "unix:$OUT/monitor.sock,server=on,wait=off" \
    "${GPU_ARGS[@]}" \
    "${SERIAL_ARGS[@]}" \
    -serial "unix:$INTERACTIVE_SERIAL_SOCKET,server=on,wait=off" \
    -display "${QEMU_DISPLAY_ARGS[0]}" \
    -D "$OUT/qemu.log"

QEMU_STATUS="$?"
echo
echo "QEMU exited with status $QEMU_STATUS"
echo "Serial log: $SERIAL_LOG"
exit "$QEMU_STATUS"
