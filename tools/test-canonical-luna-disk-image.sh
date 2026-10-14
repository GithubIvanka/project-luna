#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_ROOT="${LUNA_CANONICAL_OUTPUT_ROOT:-$REPO_ROOT/dist}"
VERSION="${LUNA_VERSION:-0.1.0}"
IMAGE="${LUNA_CANONICAL_IMAGE:-$OUTPUT_ROOT/luna-$VERSION.img}"
MESA_ROOT="${LUNA_ALPINE_GRAPHICS_BUILD_ROOT:-$REPO_ROOT/dist/.build/alpine-foundation-graphics}/mesa/out"
if [ -n "${LUNA_CANONICAL_INPUT_ROOT:-}" ]; then
    MESA_ROOT="$LUNA_CANONICAL_INPUT_ROOT/graphics/mesa/out"
fi
command -v sgdisk >/dev/null || { echo "missing sgdisk" >&2; exit 1; }
command -v blkid >/dev/null || { echo "missing blkid" >&2; exit 1; }
command -v mdir >/dev/null || { echo "missing mdir" >&2; exit 1; }
command -v debugfs >/dev/null || { echo "missing debugfs" >&2; exit 1; }
command -v btrfs >/dev/null || { echo "missing btrfs" >&2; exit 1; }
command -v fakeroot >/dev/null || { echo "missing fakeroot" >&2; exit 1; }
[ -f "$IMAGE" ] || { echo "canonical image missing: $IMAGE" >&2; exit 1; }

# Inspect the actual disk, without retaining builder exports or partition copies.
ARTIFACT_TMP="$(mktemp -d "$REPO_ROOT/target/canonical-structure.XXXXXX")"
trap 'rm -rf "$ARTIFACT_TMP"' EXIT
WORK="$ARTIFACT_TMP/disk"
python3 "$REPO_ROOT/tools/canonical-luna-artifacts.py" extract "$IMAGE" "$WORK"

echo "==> GPT"
sgdisk -p "$IMAGE"
[ "$(sgdisk -i 1 "$IMAGE" | awk -F": " '/Partition name:/ {gsub(/\047/, "", $2); print $2; exit}')" = EFI ]
[ "$(sgdisk -i 2 "$IMAGE" | awk -F": " '/Partition name:/ {gsub(/\047/, "", $2); print $2; exit}')" = LUNA-SYS ]
[ "$(sgdisk -i 3 "$IMAGE" | awk -F": " '/Partition name:/ {gsub(/\047/, "", $2); print $2; exit}')" = LUNA-DATA ]
[ "$(sgdisk -i 4 "$IMAGE" | awk -F": " '/Partition name:/ {gsub(/\047/, "", $2); print $2; exit}')" = SWAP ]

SYS_START="$(sgdisk -i 2 "$IMAGE" | awk '/First sector:/ {print $3; exit}')"
DATA_START="$(sgdisk -i 3 "$IMAGE" | awk '/First sector:/ {print $3; exit}')"
SWAP_START="$(sgdisk -i 4 "$IMAGE" | awk '/First sector:/ {print $3; exit}')"

echo "==> Filesystems"
[ "$(blkid -p -O $((2048 * 512)) -s TYPE -o value "$IMAGE")" = vfat ]
[ "$(blkid -p -O $((SYS_START * 512)) -s TYPE -o value "$IMAGE")" = ext4 ]
[ "$(blkid -p -O $((DATA_START * 512)) -s TYPE -o value "$IMAGE")" = btrfs ]
[ "$(blkid -p -O $((SWAP_START * 512)) -s TYPE -o value "$IMAGE")" = swap ]
[ "$(blkid -p -O $((SYS_START * 512)) -s LABEL -o value "$IMAGE")" = LUNA-SYS ]
[ "$(blkid -p -O $((DATA_START * 512)) -s LABEL -o value "$IMAGE")" = LUNA-DATA ]
[ "$(blkid -p -O $((SWAP_START * 512)) -s LABEL -o value "$IMAGE")" = SWAP ]

echo "==> EFI contents"
mdir -i "$WORK/parts/efi.fs" ::/EFI/LUNA | grep -q "LUNA-BOOT.EFI"

echo "==> LUNA-SYS contents"
SYS_FS="$WORK/parts/luna-sys.fs"
for path in     /kernels/7.2.4/bzImage     /cores/luna-$VERSION.init     /cores/luna-$VERSION.toml     /images/luna-$VERSION.squashfs     /images/luna-$VERSION.toml     /config/boot-state.toml     /config/luna-data.toml     /recovery/recovery.squashfs     /recovery/recovery.toml; do
    debugfs -R "stat $path" "$SYS_FS" 2>/dev/null | grep -q "Type: regular" || {
        echo "missing LUNA-SYS artifact: $path" >&2
        exit 1
    }
done

echo "==> System Image namespaces"
IMAGE_FS="$WORK/luna-$VERSION.squashfs"
for path in apps drivers firmware libs config resources; do
    unsquashfs -ll "$IMAGE_FS" 2>/dev/null | grep -Eq "squashfs-root/$path$" || {
        echo "missing System Image namespace: $path" >&2
        exit 1
    }
done
if unsquashfs -ll "$IMAGE_FS" 2>/dev/null | grep -Eq 'squashfs-root/(bin|etc|home|proc|run|sbin|sys|tmp|usr|var)(/|$)'; then
    echo "transitional Alpine root leaked into minimal System Image" >&2
    exit 1
fi
unsquashfs -ll "$IMAGE_FS" 2>/dev/null | grep -q 'squashfs-root/apps/luna-init' && {
    echo "luna-init must remain a LUNA-SYS core, not a System Image app" >&2
    exit 1
}
for path in     apps/luna-device-manager/luna-device-manager     apps/luna-volume-manager/luna-volume-manager     apps/luna-system-runtime/luna-system-runtime     apps/luna-user-session/luna-user-session     config/desktop.toml     config/group     config/passwd     config/shadow     config/nsswitch.conf     config/luna/niri-config.kdl     resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf     resources/fonts/AdwaitaSans-Regular.ttf     resources/images/wallpaper-night-sky.png     resources/images/stone-reference.png     apps/luna-graphical-session/luna-graphical-session; do
    unsquashfs -ll "$IMAGE_FS" 2>/dev/null | grep -q "squashfs-root/$path$" || {
        echo "missing System Image file: $path" >&2
        exit 1
    }
done

echo "==> LUNA-DATA contents"
python3 "$REPO_ROOT/tools/test-canonical-luna-data-ownership.py" "$WORK/parts/luna-data.fs"
TMP="$ARTIFACT_TMP/data-verification"
rm -rf "$TMP"
mkdir -p "$TMP"
btrfs restore -D "$WORK/parts/luna-data.fs" "$TMP" >/dev/null
# Owners are checked from raw inodes above. The host extraction needs modes,
# but must not attempt real chown to guest identities as the unprivileged user.
fakeroot btrfs restore -mS "$WORK/parts/luna-data.fs" "$TMP" >/dev/null
test "$(find "$TMP" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | tr '\n' ' ')" = "cache system users "
[ ! -e "$TMP/state" ] && [ ! -e "$TMP/volumes" ] && [ ! -e "$TMP/cach" ]
[ -d "$TMP/system/state" ] && [ -d "$TMP/system/volumes" ]
RECOVERY_FS="$WORK/luna-sys/recovery/recovery.squashfs"
for path in cache system/state system/volumes users/recovery/home users/recovery/config users/recovery/data; do
    unsquashfs -ll "$RECOVERY_FS" 2>/dev/null | grep -Eq "squashfs-root/$path$" || {
        echo "missing Recovery DATA namespace: $path" >&2
        exit 1
    }
done
[ -x "$TMP/system/apps/niri/niri" ]
[ -f "$TMP/system/libs/libwayland-server.so.0" ]
[ -x "$TMP/system/apps/noctalia/noctalia" ]
[ -d "$TMP/system/apps/noctalia/resources/noctalia/assets" ]
[ -n "$(find "$TMP/system/apps/noctalia/resources/noctalia/assets" -type f -print -quit)" ]
[ -x "$TMP/system/apps/openrc/openrc" ]
[ -x "$TMP/system/apps/openrc/openrc-run" ]
[ -x "$TMP/system/apps/openrc/start-stop-daemon" ]
[ -x "$TMP/system/apps/openrc/resources/rc/bin/openrc-user" ]
[ -x "$TMP/system/apps/elogind/elogind" ]
[ -x "$TMP/system/apps/elogind/elogind-uaccess-command" ]
grep -Fq '/usr/bin/elogind-uaccess-command' \
    "$TMP/system/apps/udevd/resources/udev/rules.d/73-seat-late.rules"
[ -x "$TMP/system/apps/elogind/loginctl" ]
[ -x "$TMP/system/apps/upower/upowerd" ]
[ -x "$TMP/system/apps/polkit/polkitd" ]
[ -x "$TMP/system/apps/pipewire/pipewire" ]
[ -x "$TMP/system/apps/wireplumber/wireplumber" ]
[ -x "$TMP/system/apps/dbus-daemon/dbus-run-session" ]
[ -x "$TMP/system/apps/dbus-daemon/dbus-send" ]
# Each shared resource namespace has one owner: a second bind mount over
# dbus-1/polkit-1 would hide the other providers' policies in the logical root.
for name in dbus-1 polkit-1; do
    [ "$(find "$TMP/system/apps" -path "*/resources/$name" -type d | wc -l)" -eq 1 ]
    [ -d "$TMP/system/apps/dbus-daemon/resources/$name" ]
done
for policy in org.freedesktop.login1.conf org.freedesktop.PolicyKit1.conf org.freedesktop.UPower.conf org.freedesktop.UDisks2.conf; do
    [ -s "$TMP/system/apps/dbus-daemon/resources/dbus-1/system.d/$policy" ]
done
[ -s "$TMP/system/config/UPower/UPower.conf" ]
[ -s "$TMP/system/config/polkit-1/rules.d/49-luna-power.rules" ]
grep -Fq 'subject.isInGroup("wheel")' "$TMP/system/config/polkit-1/rules.d/49-luna-power.rules"
for applet in mount halt poweroff reboot; do
    [ "$(readlink "$TMP/system/apps/busybox/$applet")" = busybox ]
done
[ -f "$TMP/system/libs/niri-compat/libudev.so.1" ]
# Check actual packaged bytes, not merely the presence of Mesa filenames.
# Alpine's generic libgallium has the same filename as the VirGL build.
MESA_PACKAGED="$TMP/system/libs/mesa-virgl"
for library in libEGL.so.1.0.0 libgbm.so.1.0.0 libgallium-26.1.6.so; do
    cmp "$MESA_ROOT/usr/local/lib/$library" "$MESA_PACKAGED/$library" || {
        echo "packaged Mesa differs from the tracked VirGL build: $library" >&2
        exit 1
    }
done
cmp "$MESA_ROOT/usr/lib/dri/libdril_dri.so" "$MESA_PACKAGED/dri/libdril_dri.so"
[ "$(readlink "$MESA_PACKAGED/dri/virtio_gpu_dri.so")" = libdril_dri.so ]
[ "$(readlink "$MESA_PACKAGED/libEGL.so.1")" = libEGL.so.1.0.0 ]
[ "$(readlink "$MESA_PACKAGED/libgbm.so.1")" = libgbm.so.1.0.0 ]
GRAPHICAL_SESSION="$(unsquashfs -cat "$IMAGE_FS" apps/luna-graphical-session/luna-graphical-session)"
for setting in \
    'export LD_LIBRARY_PATH="/usr/lib/niri-compat:/usr/lib/mesa-virgl:/usr/lib:/usr/local/lib"' \
    'export LIBGL_DRIVERS_PATH="/usr/lib/mesa-virgl/dri"' \
    'export GBM_BACKENDS_PATH="/usr/lib/mesa-virgl/gbm"'; do
    grep -Fq "$setting" <<< "$GRAPHICAL_SESSION"
done
for provider in niri/niri noctalia/noctalia elogind/elogind-uaccess-command; do
    LD_LIBRARY_PATH="$TMP/system/libs/niri-compat:$MESA_PACKAGED:$TMP/system/libs:$TMP/system/libs/elogind" \
        "$TMP/system/libs/ld-musl-x86_64.so.1" --list "$TMP/system/apps/$provider" \
        > "$TMP/loader-${provider%%/*}.log" 2>&1 || {
        echo "unresolved musl provider dependencies: $provider (see $TMP/loader-${provider%%/*}.log)" >&2
        exit 1
    }
done
BOOTSTRAP_GROUP="$(unsquashfs -cat "$IMAGE_FS" config/group)"
for group in video audio disk uucp; do
    grep -q "^$group:" <<< "$BOOTSTRAP_GROUP"
done
[ -x "$TMP/system/apps/seatd/seatd" ]
[ -x "$TMP/system/apps/dbus-daemon/dbus-daemon" ]
[ -x "$TMP/system/apps/networkmanager/NetworkManager" ]
[ -x "$TMP/system/apps/bluez/bluetoothd" ]
[ -x "$TMP/system/apps/udisksd/udisksd" ]
[ -x "$TMP/system/apps/udisksctl/udisksctl" ]
[ -x "$TMP/system/apps/udevd/udevd" ]
[ -x "$TMP/system/apps/udevd/udevadm" ]
[ -f "$TMP/system/apps/udevd/resources/udev/rules.d/60-input-id.rules" ]
[ -d "$TMP/system/apps/niri/resources/libinput" ]
[ -n "$(find "$TMP/system/apps/niri/resources/libinput" -type f -print -quit)" ]
[ -d "$TMP/system/apps/niri/resources/xkeyboard-config-2" ]
[ -f "$TMP/system/apps/niri/resources/xkeyboard-config-2/rules/evdev" ]
[ -d "$TMP/system/resources/icons/Adwaita/cursors" ]
[ -n "$(find "$TMP/system/resources/icons/Adwaita/cursors" -type f -print -quit)" ]
[ -f "$TMP/system/config/fonts/fonts.conf" ]
[ -s "$TMP/system/resources/fonts/Adwaita/AdwaitaSans-Regular.ttf" ]
[ -s "$TMP/system/resources/fonts/Adwaita/AdwaitaMono-Regular.ttf" ]
[ -s "$TMP/system/apps/noctalia/resources/qalculate/units.xml" ]
[ -f "$TMP/system/config/rc.conf" ]
[ -x "$TMP/system/config/init.d/luna-device-manager" ]
[ -x "$TMP/system/config/init.d/luna-volume-manager" ]
[ ! -e "$TMP/system/config/runlevels/default/luna-device-manager" ]
[ ! -L "$TMP/system/config/runlevels/default/luna-device-manager" ]
[ ! -e "$TMP/system/config/runlevels/default/luna-volume-manager" ]
[ ! -L "$TMP/system/config/runlevels/default/luna-volume-manager" ]
[ -x "$TMP/system/config/init.d/dbus" ]
[ -x "$TMP/system/config/init.d/elogind" ]
[ -x "$TMP/system/config/init.d/luna-session" ]
[ -d "$TMP/system/config/runlevels/default" ]
RUNLEVEL_ROOT="$TMP/system/config/runlevels/default"
[ -L "$RUNLEVEL_ROOT/dbus" ]
[ -L "$RUNLEVEL_ROOT/elogind" ]
[ -L "$RUNLEVEL_ROOT/networkmanager" ]
[ -d "$TMP/system/apps/openrc/resources/rc" ]
[ -d "$TMP/system/apps/openrc/resources/rc/sh" ]
[ -d "$TMP/system/apps/openrc/resources/rc/sbin" ]
[ -d "$TMP/system/apps/niri/resources/X11" ]
[ -d "$TMP/system/apps/niri/resources/X11/xkb" ]
[ -f "$TMP/users/luna/config/niri/config.kdl" ]
[ -d "$TMP/users/luna/home" ] && [ -d "$TMP/users/luna/data" ] && [ -d "$TMP/users/luna/config" ]

echo "Canonical Luna disk verification: PASS"
echo "Image: $IMAGE"
