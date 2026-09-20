#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
KERNEL="${LUNA_TEST_KERNEL:-${REPO_ROOT}/dist/kernel/current/bzImage}"
DESKTOP_ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/.build/desktop-payload}"

for tool in mkfs.ext4 mkfs.fat mkfs.btrfs mkswap dd sgdisk mksquashfs qemu-system-x86_64; do
    command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done

[ -f "$KERNEL" ] || { echo "missing kernel: $KERNEL" >&2; exit 1; }
[ -x "$DESKTOP_ROOT/usr/bin/luna-user-session" ] || { echo "missing luna-user-session" >&2; exit 1; }
[ -x "$DESKTOP_ROOT/usr/bin/niri" ] || { echo "missing niri" >&2; exit 1; }

printf 'image-build prerequisites: OK\n'
printf 'mkfs.btrfs: %s\n' "$(command -v mkfs.btrfs)"
printf 'kernel: %s\n' "$KERNEL"
printf 'desktop-root: %s\n' "$DESKTOP_ROOT"
printf 'output: %s\n' "$OUT"

