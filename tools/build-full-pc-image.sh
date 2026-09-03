#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
DESKTOP_ROOT="${LUNA_DESKTOP_ROOT:-${DIST}/desktop-root}"
KERNEL_ROOT="${LUNA_KERNEL_OUT:-${DIST}/kernel}"
KERNEL="${LUNA_TEST_KERNEL:-${KERNEL_ROOT}/current/bzImage}"
BUSYBOX="${BUSYBOX:-}"

cd "$REPO_ROOT"

find_busybox() {
    for candidate in /usr/bin/busybox /bin/busybox; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

if [ -z "$BUSYBOX" ]; then
    BUSYBOX="$(find_busybox || true)"
fi

: "${BUSYBOX:?Ошибка: не найден статический BusyBox. Укажите BUSYBOX=/path/to/busybox.}"

for command_name in cargo rustup curl git make meson ninja zig cmake pkg-config ldd python3 sgdisk mkfs.ext4 mkfs.fat mcopy mmd dd mksquashfs cpio gzip file; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "Ошибка: не найден обязательный инструмент: $command_name" >&2
        exit 1
    }
done

printf '%s\n' '=== 1/6: Linux kernel ==='
bash tools/build-luna-kernel.sh

[ -f "$KERNEL" ] || {
    echo "Ошибка: после сборки kernel не найден: $KERNEL" >&2
    exit 1
}

printf '%s\n' '=== 2/6: графический System Image root ==='
bash tools/build-desktop-root.sh
[ -d "$DESKTOP_ROOT" ] || {
    echo "Ошибка: desktop root не создан: $DESKTOP_ROOT" >&2
    exit 1
}

printf '%s\n' '=== 3/6: Yazi + Luna Files ==='
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" bash tools/build-yazi-payload.sh

printf '%s\n' '=== 4/6: системные desktop-службы ==='
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" bash tools/prepare-desktop-services.sh

printf '%s\n' '=== 5/6: финальная сессия niri ==='
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" bash tools/patch-niri-session.sh

printf '%s\n' '=== 6/6: EFI + SYSTEM + DATA PC image ==='
LUNA_TEST_KERNEL="$KERNEL" \
BUSYBOX="$BUSYBOX" \
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" \
bash tools/build-pc-image.sh

printf '\nГотово. Полный образ: %s\n' "$DIST/luna-pc.img"
printf 'System Image: %s\n' "$DIST/luna-${LUNA_VERSION:-0.1.0}.squashfs"
printf 'Kernel: %s\n' "$KERNEL"
printf 'Desktop root: %s\n' "$DESKTOP_ROOT"
