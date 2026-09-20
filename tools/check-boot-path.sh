#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${LUNA_DIST_DIR:-$REPO_ROOT/dist}"
SYS="$DIST/luna-sys"
IMAGE="${LUNA_SYSTEM_IMAGE:-$(find "$DIST" -maxdepth 1 -type f -name 'luna-*.squashfs' -print -quit)}"
KERNEL="$DIST/kernel/current/bzImage"
FAIL=0

check() {
    local label="$1" path="$2"
    if [ -e "$path" ]; then
        printf 'OK   %-34s %s\\n' "$label" "$path"
    else
        printf 'FAIL %-34s %s\\n' "$label" "$path" >&2
        FAIL=1
    fi
}

printf '%s\\n' 'Project Luna boot-path audit'
printf '%s\\n' '============================'

[ -n "$IMAGE" ] || { echo 'FAIL no System Image found' >&2; exit 1; }
check 'LUNA-SYS root' "$SYS"
check 'System Image' "$IMAGE"
check 'kernel' "$KERNEL"
for path in images cores kernels config recovery; do check "LUNA-SYS/$path" "$SYS/$path"; done
for path in config/boot-state.toml config/luna-data.toml images/"$(basename "$IMAGE" .squashfs)".toml; do check "system config $path" "$SYS/$path"; done

check 'recovery DATA image' "$SYS/recovery/recovery.squashfs"
check 'recovery DATA manifest' "$SYS/recovery/recovery.toml"

unsquashfs -ll "$IMAGE" 2>/dev/null > "$DIST/.build/boot-path-system-image.lst"
LIST="$DIST/.build/boot-path-system-image.lst"
for path in \
    'apps/luna-device-manager/luna-device-manager' \
    'apps/luna-system-runtime/luna-system-runtime' \
    'apps/luna-user-session/luna-user-session' \
    'config/passwd' 'config/group' 'config/shadow' 'config/nsswitch.conf' \
    'config/luna/desktop.toml'; do
    grep -q "squashfs-root/$path$" "$LIST" && printf 'OK   image %-29s %s\\n' "$path" || { printf 'FAIL image %-27s %s\\n' "$path" "$path" >&2; FAIL=1; }
done
for class in fonts icons themes cursors sounds locales translations; do
    grep -q "squashfs-root/resources/$class$" "$LIST" && printf 'OK   resource %-25s %s\\n' "$class" "$class" || { printf 'FAIL resource %-23s %s\\n' "$class" "$class" >&2; FAIL=1; }
done
for path in \
    'apps/unix_chkpwd/unix_chkpwd' \
    'libs/loader/ld-linux-x86-64.so.2' \
    'apps/greetd/greetd' \
    'apps/noctalia-greeter-session/noctalia-greeter-session' \
    'apps/niri/niri' \
    'apps/dbus-daemon/dbus-daemon'; do
    if grep -q "squashfs-root/$path$" "$LIST"; then
        printf 'FAIL forbidden image payload %-18s %s\\n' "$path" "$path" >&2
        FAIL=1
    else
        printf 'OK   absent from image %-24s %s\\n' "$path" "$path"
    fi
done

unsquashfs -ll "$SYS/recovery/recovery.squashfs" 2>/dev/null > "$DIST/.build/boot-path-recovery-image.lst"
RECOVERY_LIST="$DIST/.build/boot-path-recovery-image.lst"
for path in \
    'system/apps/niri/niri' \
    'system/apps/noctalia/noctalia' \
    'system/apps/ghostty/ghostty' \
    'system/libs/loader/ld-linux-x86-64.so.2' \
    'system/libs/x86_64-linux-gnu/libc.so.6' \
    'system/config/luna/desktop.toml'; do
    grep -q "squashfs-root/$path$" "$RECOVERY_LIST" && printf 'OK   recovery %-24s %s\n' "$path" || { printf 'FAIL recovery %-22s %s\n' "$path" "$path" >&2; FAIL=1; }
done
for path in \
    'users/recovery/home' \
    'users/recovery/data' \
    'users/recovery/config' \
    'system/state/auth/passwd' \
    'system/state/auth/group' \
    'system/state/auth/shadow'; do
    grep -q "squashfs-root/$path$" "$RECOVERY_LIST" && printf 'OK   recovery %-24s %s\n' "$path" || { printf 'FAIL recovery %-22s %s\n' "$path" "$path" >&2; FAIL=1; }
done
for path in \
    'system/apps/greetd/greetd' \
    'system/apps/noctalia-greeter-session/noctalia-greeter-session' \
    'system/apps/seatd/seatd' \
    'system/apps/dbus-run-session/dbus-run-session' \
    'system/apps/unix_chkpwd/unix_chkpwd'; do
    if grep -q "squashfs-root/$path$" "$RECOVERY_LIST"; then
        printf 'FAIL recovery login provider %-12s %s\n' "$path" "$path" >&2
        FAIL=1
    else
        printf 'OK   recovery has no login provider %s\n' "$path"
    fi
done

DATA="$DIST/luna-data"
check 'LUNA-DATA root' "$DATA"
for path in \
    'system/apps/niri/niri' \
    'system/apps/noctalia/noctalia' \
    'system/apps/ghostty/ghostty' \
    'system/libs/loader/ld-linux-x86-64.so.2'; do
    check "DATA/$path" "$DATA/$path"
done
for path in \
    'system/apps/greetd/greetd' \
    'system/apps/noctalia-greeter-session/noctalia-greeter-session' \
    'system/apps/seatd/seatd' \
    'system/apps/dbus-run-session/dbus-run-session' \
    'system/apps/unix_chkpwd/unix_chkpwd' \
    'system/apps/niri-session/niri-session'; do
    if [ -e "$DATA/$path" ]; then
        printf 'FAIL forbidden DATA login/helper %s\\n' "$path" >&2
        FAIL=1
    else
        printf 'OK   DATA has no legacy login/helper %s\\n' "$path"
    fi
done
for path in \
    'system/apps/niri/resources/wayland-sessions' \
    'system/apps/noctalia/resources/applications' \
    'system/apps/dbus-daemon/resources/dbus-1' \
    'system/apps/dbus-daemon/resources/polkit-1'; do
    check "DATA/$path" "$DATA/$path"
done

for path in \
    'UEFI -> luna-boot.efi' \
    'kernel -> luna-init via setup_data' \
    'luna-init -> luna-system-runtime' \
    'luna-system-runtime -> UserSession' \
    'UserSession -> native identity authentication' \
    'UserSession -> future native SessionUI credential entry' \
    'normal -> PhysicalData' \
    'recovery -> VirtualData'; do
    printf 'PATH %s\\n' "$path"
done

if [ "$FAIL" -ne 0 ]; then
    echo 'Boot-path audit: FAILED' >&2
    exit 1
fi
echo 'Boot-path audit: PASSED'
