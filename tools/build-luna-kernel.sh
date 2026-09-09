#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_KERNEL_OUT:-${REPO_ROOT}/dist/kernel}"
VERSION="${LUNA_KERNEL_VERSION:-7.2.4}"
JOBS="${LUNA_KERNEL_JOBS:-$(nproc)}"
SRC="${OUT}/linux-${VERSION}"
TARBALL="${OUT}/linux-${VERSION}.tar.xz"
URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${VERSION}.tar.xz"
CONFIG_FRAGMENT="${REPO_ROOT}/kernel/luna-x86_64.config"
PATCH_DIR="${REPO_ROOT}/kernel/patches"

for tool in curl tar make patch; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
[ -f "$CONFIG_FRAGMENT" ] || { echo "missing kernel config: $CONFIG_FRAGMENT" >&2; exit 1; }

mkdir -p "$OUT"
if [ ! -d "$SRC" ]; then
    if [ ! -f "$TARBALL" ]; then
        curl --fail --location --retry 3 --output "$TARBALL" "$URL"
    fi
    tar -xJf "$TARBALL" -C "$OUT"
fi

cd "$SRC"

# Keep the source tree reproducible: a previously patched tree must never be
# silently reused for a clean build.
if [ -e .luna-patches-applied ]; then
    echo "refusing to reuse already patched kernel source: $SRC" >&2
    echo "remove $SRC or build with a fresh LUNA_KERNEL_OUT" >&2
    exit 1
fi

if [ -d "$PATCH_DIR" ]; then
    shopt -s nullglob
    PATCHES=("$PATCH_DIR"/*.patch)
    shopt -u nullglob
    for patch in "${PATCHES[@]}"; do
        echo "Applying Luna kernel patch: $(basename "$patch")"
        patch --fuzz=0 -p1 --forward --batch < "$patch"
    done
fi

touch .luna-patches-applied

make O="$SRC/build" ARCH=x86_64 x86_64_defconfig
cat "$CONFIG_FRAGMENT" >> "$SRC/build/.config"
make O="$SRC/build" ARCH=x86_64 olddefconfig

if ! grep -q '^CONFIG_RUST=y$' "$SRC/build/.config"; then
    echo "Project Luna requires CONFIG_RUST=y after olddefconfig" >&2
    exit 1
fi

make O="$SRC/build" ARCH=x86_64 rustavailable
make O="$SRC/build" ARCH=x86_64 -j"$JOBS" bzImage modules

KERNEL_RELEASE="$(make O="$SRC/build" ARCH=x86_64 -s kernelrelease)"
mkdir -p "$OUT/$KERNEL_RELEASE"
cp "$SRC/build/arch/x86/boot/bzImage" "$OUT/$KERNEL_RELEASE/bzImage"
cp "$SRC/build/System.map" "$OUT/$KERNEL_RELEASE/System.map"
cp "$SRC/build/.config" "$OUT/$KERNEL_RELEASE/config"
printf '%s\n' "$KERNEL_RELEASE" > "$OUT/$KERNEL_RELEASE/release"
ln -sfn "$KERNEL_RELEASE" "$OUT/current"

echo "Built Project Luna kernel: $OUT/$KERNEL_RELEASE/bzImage"
