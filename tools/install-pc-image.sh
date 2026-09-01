#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-${REPO_ROOT}/dist/luna-pc.img}"
DEVICE="${2:-}"

usage() {
    echo "Usage: sudo $0 <luna-pc.img> <whole-disk-device> --yes" >&2
    echo "Example: sudo $0 dist/luna-pc.img /dev/nvme0n1 --yes" >&2
    exit 2
}

[ "${3:-}" = "--yes" ] || usage
[ -f "$IMAGE" ] || { echo "image not found: $IMAGE" >&2; exit 1; }
[ -n "$DEVICE" ] || usage

case "$DEVICE" in
    /dev/*) ;;
    *) echo "refusing non-device target: $DEVICE" >&2; exit 1 ;;
esac

[ -b "$DEVICE" ] || { echo "target is not a block device: $DEVICE" >&2; exit 1; }

command -v lsblk >/dev/null || { echo "missing required tool: lsblk" >&2; exit 1; }
command -v dd >/dev/null || { echo "missing required tool: dd" >&2; exit 1; }
command -v blockdev >/dev/null || { echo "missing required tool: blockdev" >&2; exit 1; }

if lsblk -nrpo NAME,MOUNTPOINT "$DEVICE" | awk 'NF >= 2 && $2 != "" { found=1 } END { exit(found ? 0 : 1) }'; then
    echo "refusing to overwrite a disk with mounted filesystems: $DEVICE" >&2
    lsblk -o NAME,SIZE,FSTYPE,LABEL,MOUNTPOINTS "$DEVICE" >&2
    exit 1
fi

IMAGE_BYTES=$(stat -c '%s' "$IMAGE")
DEVICE_BYTES=$(blockdev --getsize64 "$DEVICE")
if [ "$IMAGE_BYTES" -gt "$DEVICE_BYTES" ]; then
    echo "image is larger than target disk" >&2
    echo "image: ${IMAGE_BYTES} bytes, target: ${DEVICE_BYTES} bytes" >&2
    exit 1
fi

echo "About to overwrite the ENTIRE disk: $DEVICE"
echo "Image: $IMAGE"
lsblk -o NAME,SIZE,MODEL,SERIAL,FSTYPE,LABEL,MOUNTPOINTS "$DEVICE" >&2 || true
printf 'Type ERASE-LUNA to continue: '
read -r confirmation
[ "$confirmation" = "ERASE-LUNA" ] || { echo "aborted" >&2; exit 1; }

echo "Writing Luna image..." >&2
dd if="$IMAGE" of="$DEVICE" bs=16M conv=fsync status=progress
sync

echo "Luna image written successfully to $DEVICE." >&2
echo "Power off before removing the installation media, then boot the target disk in UEFI mode." >&2
