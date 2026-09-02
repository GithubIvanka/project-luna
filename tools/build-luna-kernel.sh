#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_KERNEL_OUT:-${REPO_ROOT}/dist/kernel}"
VERSION="${LUNA_KERNEL_VERSION:-7.2.2}"
JOBS="${LUNA_KERNEL_JOBS:-$(nproc)}"
SRC="${OUT}/linux-${VERSION}"
TARBALL="${OUT}/linux-${VERSION}.tar.xz"
URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${VERSION}.tar.xz"
CONFIG_FRAGMENT="${REPO_ROOT}/kernel/luna-x86_64.config"

for tool in curl tar make; do
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
make O="$SRC/build" ARCH=x86_64 x86_64_defconfig
cat "$CONFIG_FRAGMENT" >> "$SRC/build/.config"
make O="$SRC/build" ARCH=x86_64 olddefconfig
make O="$SRC/build" ARCH=x86_64 -j"$JOBS" bzImage modules

KERNEL_RELEASE="$(make O="$SRC/build" ARCH=x86_64 -s kernelrelease)"
mkdir -p "$OUT/$KERNEL_RELEASE"
cp "$SRC/build/arch/x86/boot/bzImage" "$OUT/$KERNEL_RELEASE/bzImage"
cp "$SRC/build/System.map" "$OUT/$KERNEL_RELEASE/System.map"
cp "$SRC/build/.config" "$OUT/$KERNEL_RELEASE/config"
printf '%s\n' "$KERNEL_RELEASE" > "$OUT/$KERNEL_RELEASE/release"
ln -sfn "$KERNEL_RELEASE" "$OUT/current"

echo "Built Project Luna kernel: $OUT/$KERNEL_RELEASE/bzImage"
