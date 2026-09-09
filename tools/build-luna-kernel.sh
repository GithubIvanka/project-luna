#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_KERNEL_OUT:-${REPO_ROOT}/dist/kernel}"
VERSION="${LUNA_KERNEL_VERSION:-7.2.4}"
JOBS="${LUNA_KERNEL_JOBS:-$(nproc)}"
LLVM_MODE="${LUNA_KERNEL_LLVM:-1}"
SRC="${OUT}/linux-${VERSION}"
TARBALL="${OUT}/linux-${VERSION}.tar.xz"
URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${VERSION}.tar.xz"
CONFIG_FRAGMENT="${REPO_ROOT}/kernel/luna-x86_64.config"
OVERLAY="${REPO_ROOT}/tools/apply-luna-kernel-overlay.sh"

for tool in curl tar make python3; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
if [ "$LLVM_MODE" = "1" ]; then
    command -v clang >/dev/null || { echo "missing required tool: clang" >&2; exit 1; }
    command -v ld.lld >/dev/null || { echo "missing required tool: ld.lld" >&2; exit 1; }
fi
[ -f "$CONFIG_FRAGMENT" ] || { echo "missing kernel config: $CONFIG_FRAGMENT" >&2; exit 1; }
[ -f "$OVERLAY" ] || { echo "missing kernel overlay: $OVERLAY" >&2; exit 1; }

mkdir -p "$OUT"
if [ ! -d "$SRC" ]; then
    if [ ! -f "$TARBALL" ]; then
        curl --fail --location --retry 3 --output "$TARBALL" "$URL"
    fi
    tar -xJf "$TARBALL" -C "$OUT"
fi

cd "$SRC"

# A source tree that has already received the Luna overlay is never silently
# reused. Delete it (or use another LUNA_KERNEL_OUT) for a clean application.
if [ -e .luna-overlay-applied ]; then
    echo "refusing to reuse an already overlaid kernel source: $SRC" >&2
    echo "remove $SRC or build with a fresh LUNA_KERNEL_OUT" >&2
    exit 1
fi

bash "$OVERLAY" "$SRC" "$REPO_ROOT"
touch .luna-overlay-applied

MAKE=(make)
if [ "$LLVM_MODE" = "1" ]; then
    MAKE+=(LLVM=1)
fi
MAKE+=(O="$SRC/build" ARCH=x86_64)

"${MAKE[@]}" x86_64_defconfig
cat "$CONFIG_FRAGMENT" >> "$SRC/build/.config"
"${MAKE[@]}" olddefconfig

if ! grep -q '^CONFIG_RUST=y$' "$SRC/build/.config"; then
    echo "Project Luna requires CONFIG_RUST=y after olddefconfig" >&2
    exit 1
fi

"${MAKE[@]}" rustavailable
"${MAKE[@]}" -j"$JOBS" bzImage modules

KERNEL_RELEASE="$("${MAKE[@]}" -s kernelrelease)"
mkdir -p "$OUT/$KERNEL_RELEASE"
cp "$SRC/build/arch/x86/boot/bzImage" "$OUT/$KERNEL_RELEASE/bzImage"
cp "$SRC/build/System.map" "$OUT/$KERNEL_RELEASE/System.map"
cp "$SRC/build/.config" "$OUT/$KERNEL_RELEASE/config"
printf '%s\n' "$KERNEL_RELEASE" > "$OUT/$KERNEL_RELEASE/release"
ln -sfn "$KERNEL_RELEASE" "$OUT/current"

echo "Built Project Luna kernel: $OUT/$KERNEL_RELEASE/bzImage"
