#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
DESKTOP_ROOT="${LUNA_DESKTOP_ROOT:-${DIST}/.build/desktop-payload}"
KERNEL_ROOT="${LUNA_KERNEL_OUT:-${DIST}/kernel}"
KERNEL="${LUNA_TEST_KERNEL:-${KERNEL_ROOT}/current/bzImage}"
DESKTOP_DEV_ROOT="${LUNA_DESKTOP_DEV_ROOT:-${DIST}/.build/dev}"
cd "$REPO_ROOT"

for command_name in cargo rustup curl git make meson ninja cmake pkg-config ldd sgdisk mkfs.ext4 mkfs.fat mcopy mmd dd mksquashfs file; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "Ошибка: не найден обязательный инструмент: $command_name" >&2
        exit 1
    }
done

MKFS_BTRFS="${LUNA_MKFS_BTRFS:-$(command -v mkfs.btrfs || true)}"
: "${MKFS_BTRFS:?Ошибка: не найден mkfs.btrfs; установите btrfs-progs или задайте LUNA_MKFS_BTRFS=/path/to/mkfs.btrfs.}"
[ -x "$MKFS_BTRFS" ] || { echo "Ошибка: LUNA_MKFS_BTRFS не исполняемый: $MKFS_BTRFS" >&2; exit 1; }

printf '%s\n' '=== 1/6: ядро Luna (Linux kernel) ==='
if [ "${LUNA_REBUILD_KERNEL:-0}" = "1" ] || [ ! -f "$KERNEL" ]; then
    rm -rf "$KERNEL_ROOT/linux-${LUNA_KERNEL_VERSION:-7.2.4}"
    bash tools/build-luna-kernel.sh
else
    echo "Используется свежий kernel artifact: $KERNEL"
fi

[ -f "$KERNEL" ] || {
    echo "Ошибка: после подготовки kernel не найден: $KERNEL" >&2
    exit 1
}

printf '%s\n' '=== 2/6: графическая среда Luna ==='
LUNA_DESKTOP_ROOT_OUT="$DESKTOP_ROOT" \
LUNA_DESKTOP_DEV_ROOT_OUT="$DESKTOP_DEV_ROOT" \
bash tools/prepare-desktop-dev-roots.sh
export LUNA_DESKTOP_ROOT_OUT="$DESKTOP_ROOT"
export LUNA_PIPEWIRE_DEV_ROOT="$DESKTOP_DEV_ROOT/pipewire"
export LUNA_NIRI_DEV_ROOT="$DESKTOP_DEV_ROOT/niri"
export LUNA_SDBUS_DEV_ROOT="$DESKTOP_DEV_ROOT/sdbus"
export LUNA_LIBRSVG_DEV_ROOT="$DESKTOP_DEV_ROOT/librsvg"
export LUNA_NOCTALIA_DEV_ROOT="$DESKTOP_DEV_ROOT/noctalia"
export LUNA_GMP_DEV_ROOT="$DESKTOP_DEV_ROOT/gmp"
export LUNA_MPFR_DEV_ROOT="$DESKTOP_DEV_ROOT/mpfr"
export LUNA_GTK4_LAYER_SHELL_DEV_ROOT="$DESKTOP_DEV_ROOT/gtk4-layer-shell"
bash tools/build-desktop-root.sh
[ -d "$DESKTOP_ROOT" ] || {
    echo "Ошибка: desktop root не создан: $DESKTOP_ROOT" >&2
    exit 1
}

printf '%s\n' '=== 3/6: Yazi + Luna Files ==='
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" bash tools/build-yazi-payload.sh

printf '%s\n' '=== 4/6: системные desktop-службы ==='
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" bash tools/prepare-desktop-services.sh

printf '%s\n' '=== 5/6: final native Niri session ==='

printf '%s\n' '=== 6/6: EFI + SYSTEM + DATA PC image ==='
LUNA_TEST_KERNEL="$KERNEL" \
LUNA_MKFS_BTRFS="$MKFS_BTRFS" \
LUNA_DESKTOP_ROOT="$DESKTOP_ROOT" \
bash tools/build-pc-image.sh

printf '\nГотово. Полный образ: %s\n' "$DIST/luna-pc.img"
printf 'System Image: %s\n' "$DIST/luna-${LUNA_VERSION:-0.1.0}.squashfs"
printf 'Kernel: %s\n' "$KERNEL"
printf 'Desktop root: %s\n' "$DESKTOP_ROOT"
