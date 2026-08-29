#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
OUT="$ROOT/initramfs-test.img"
STAGE="$ROOT/.root"

if ! command -v cpio >/dev/null 2>&1; then
    echo "error: cpio is required" >&2
    exit 1
fi
if ! command -v gzip >/dev/null 2>&1; then
    echo "error: gzip is required" >&2
    exit 1
fi
if ! command -v busybox >/dev/null 2>&1; then
    echo "error: busybox is required" >&2
    echo "install it with: sudo apt install busybox-static" >&2
    exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/dev" "$STAGE/proc" "$STAGE/sys"
cp "$ROOT/init" "$STAGE/init"
chmod 0755 "$STAGE/init"
cp "$(command -v busybox)" "$STAGE/bin/busybox"
chmod 0755 "$STAGE/bin/busybox"
ln -s busybox "$STAGE/bin/sh"

(
    cd "$STAGE"
    find . -print | cpio -o -H newc
) | gzip -n > "$OUT"

rm -rf "$STAGE"
echo "created $OUT"
