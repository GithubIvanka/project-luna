#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="${LUNA_ONEFILE_DIRECT_OUT:-${ROOT}/out/luna-direct-kernel}"
KERNEL_VERSION="${LUNA_EXPERIMENT_KERNEL_VERSION:-7.2.4}"
KERNEL_URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${KERNEL_VERSION}.tar.xz"
TARBALL="$OUT/linux-${KERNEL_VERSION}.tar.xz"
SRC="$OUT/linux-${KERNEL_VERSION}"
BUILD="$OUT/build"
JOBS="${LUNA_EXPERIMENT_JOBS:-$(nproc)}"

for tool in curl tar make cargo rustup cc; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

mkdir -p "$OUT"
if [ ! -f "$TARBALL" ]; then
  curl --fail --location --retry 3 --output "$TARBALL" "$KERNEL_URL"
fi
if [ ! -d "$SRC" ]; then
  tar -xJf "$TARBALL" -C "$OUT"
fi

if [ -e "$SRC/.luna-overlay-applied" ]; then
  rm -rf "$SRC"
  tar -xJf "$TARBALL" -C "$OUT"
fi

echo "[1/4] Applying Luna kernel overlay"
bash "$REPO_ROOT/tools/apply-luna-kernel-overlay.sh" "$SRC" "$REPO_ROOT"
touch "$SRC/.luna-overlay-applied"

echo "[2/4] Preparing direct-init kernel config"
rm -rf "$BUILD"
make -C "$SRC" ARCH=x86_64 mrproper >/dev/null
make -C "$SRC" O="$BUILD" ARCH=x86_64 tinyconfig >/dev/null
make -C "$SRC" O="$BUILD" ARCH=x86_64 rustavailable >/dev/null

CONFIG="$BUILD/.config"
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_RUST
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_ACPI
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_RELOCATABLE
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_DEVTMPFS
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_DEVTMPFS_MOUNT
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_PROC_FS
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_SYSFS
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_TMPFS
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_BINFMT_ELF
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_TTY
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_SERIAL_8250
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_SERIAL_8250_CONSOLE
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_SERIAL_CORE
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_SERIAL_CORE_CONSOLE
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_PRINTK
"$SRC/scripts/config" --file "$CONFIG" --enable CONFIG_CMDLINE_BOOL
"$SRC/scripts/config" --file "$CONFIG" --set-str CONFIG_CMDLINE "console=ttyS0"

printf "\n%.0s" {1..256} | make -C "$SRC" O="$BUILD" ARCH=x86_64 oldconfig >/dev/null
make -C "$SRC" O="$BUILD" ARCH=x86_64 olddefconfig >/dev/null

if grep -q "^CONFIG_INITRAMFS_SOURCE=" "$CONFIG"; then
  echo "direct-init kernel unexpectedly contains CONFIG_INITRAMFS_SOURCE" >&2
  exit 1
fi
grep -q "^CONFIG_RUST=y$" "$CONFIG"
grep -q "^CONFIG_RELOCATABLE=y$" "$CONFIG"

echo "[3/4] Building direct-init bzImage"
make -C "$SRC" O="$BUILD" ARCH=x86_64 -j"$JOBS" bzImage >/dev/null

echo "[4/4] Publishing artifact"
install -Dm644 "$BUILD/arch/x86/boot/bzImage" "$ROOT/out/luna-onefile-direct.efi"
install -Dm644 "$CONFIG" "$ROOT/out/luna-onefile-direct.config"
file "$ROOT/out/luna-onefile-direct.efi"
echo "Built: $ROOT/out/luna-onefile-direct.efi"
