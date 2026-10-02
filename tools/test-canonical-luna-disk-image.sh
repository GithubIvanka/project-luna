#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="${LUNA_CANONICAL_IMAGE:-$REPO_ROOT/dist/luna-0.1.0.img}"
command -v sgdisk >/dev/null || { echo "missing sgdisk" >&2; exit 1; }
command -v blkid >/dev/null || { echo "missing blkid" >&2; exit 1; }
command -v mdir >/dev/null || { echo "missing mdir" >&2; exit 1; }
command -v debugfs >/dev/null || { echo "missing debugfs" >&2; exit 1; }
command -v btrfs >/dev/null || { echo "missing btrfs" >&2; exit 1; }
[ -f "$IMAGE" ] || { echo "canonical image missing: $IMAGE" >&2; exit 1; }

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
mdir -i "$REPO_ROOT/dist/.build/canonical-luna-disk/parts/efi.fs" ::/EFI/LUNA | grep -q "LUNA-BOOT.EFI"

echo "==> LUNA-SYS contents"
SYS_FS="$REPO_ROOT/dist/.build/canonical-luna-disk/parts/luna-sys.fs"
for path in     /kernels/7.2.4/bzImage     /cores/luna-0.1.0.init     /cores/luna-0.1.0.toml     /images/luna-0.1.0.squashfs     /images/luna-0.1.0.toml     /config/boot-state.toml     /config/luna-data.toml     /recovery/recovery.squashfs     /recovery/recovery.toml; do
    debugfs -R "stat $path" "$SYS_FS" 2>/dev/null | grep -q "Type: regular" || {
        echo "missing LUNA-SYS artifact: $path" >&2
        exit 1
    }
done

echo "==> System Image namespaces"
IMAGE_FS="$REPO_ROOT/dist/.build/canonical-luna-disk/luna-0.1.0.squashfs"
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
for path in     apps/luna-device-manager/luna-device-manager     apps/luna-system-runtime/luna-system-runtime     apps/luna-user-session/luna-user-session     config/desktop.toml     config/group     config/passwd     config/shadow     config/nsswitch.conf     config/luna/niri-config.kdl     resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf     resources/images/luna.png     apps/luna-graphical-session/luna-graphical-session; do
    unsquashfs -ll "$IMAGE_FS" 2>/dev/null | grep -q "squashfs-root/$path$" || {
        echo "missing System Image file: $path" >&2
        exit 1
    }
done

echo "==> LUNA-DATA contents"
TMP="$REPO_ROOT/dist/.build/canonical-luna-disk-verification"
rm -rf "$TMP"
mkdir -p "$TMP"
btrfs restore -mD "$REPO_ROOT/dist/.build/canonical-luna-disk/parts/luna-data.fs" "$TMP" >/dev/null
btrfs restore -m "$REPO_ROOT/dist/.build/canonical-luna-disk/parts/luna-data.fs" "$TMP" >/dev/null
test "$(find "$TMP" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | tr '\n' ' ')" = "cach system users "
[ ! -e "$TMP/state" ] && [ ! -e "$TMP/volumes" ] && [ ! -e "$TMP/system/state" ] && [ ! -e "$TMP/system/volumes" ]
[ -x "$TMP/system/apps/niri/niri" ]
[ -x "$TMP/system/apps/openrc/openrc" ]
[ -x "$TMP/system/apps/openrc/openrc-run" ]
[ -x "$TMP/system/apps/openrc/start-stop-daemon" ]
[ -x "$TMP/system/apps/openrc/resources/rc/bin/openrc-user" ]
[ -x "$TMP/system/apps/elogind/elogind" ]
[ -x "$TMP/system/apps/elogind/loginctl" ]
[ -x "$TMP/system/apps/upower/upowerd" ]
[ -x "$TMP/system/apps/polkit/polkitd" ]
[ -x "$TMP/system/apps/pipewire/pipewire" ]
[ -x "$TMP/system/apps/wireplumber/wireplumber" ]
[ -x "$TMP/system/apps/dbus-daemon/dbus-run-session" ]
[ -x "$TMP/system/apps/dbus-daemon/dbus-send" ]
[ -f "$TMP/system/libs/libgallium-26.1.6.so" ]
[ -f "$TMP/system/libs/niri-compat/libudev.so.1" ]
[ -f "$TMP/system/libs/dri/libdril_dri.so" ]
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
[ -f "$TMP/system/config/rc.conf" ]
[ -x "$TMP/system/config/init.d/dbus" ]
[ -x "$TMP/system/config/init.d/elogind" ]
[ -x "$TMP/system/config/init.d/luna-session" ]
[ -d "$TMP/system/config/runlevels/default" ]
RUNLEVEL_ROOT="$REPO_ROOT/dist/.build/canonical-luna-disk/luna-data/system/config/runlevels/default"
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
