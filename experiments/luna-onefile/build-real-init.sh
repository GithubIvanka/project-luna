#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
OUT="${LUNA_ONEFILE_REAL_OUT:-${ROOT}/out/luna-real-init-kernel}"
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

echo "[1/5] Building real musl luna-init"
cargo build --release --manifest-path "$REPO_ROOT/components/core/luna-init/Cargo.toml" --target x86_64-unknown-linux-musl
LUNA_INIT="$REPO_ROOT/components/core/luna-init/target/x86_64-unknown-linux-musl/release/luna-init"
"$REPO_ROOT/tools/check-luna-init-static.sh" "$LUNA_INIT"

echo "[2/5] Applying Luna kernel overlay"
bash "$REPO_ROOT/tools/apply-luna-kernel-overlay.sh" "$SRC" "$REPO_ROOT"
touch "$SRC/.luna-overlay-applied"

echo "[3/5] Preparing direct-init + storage kernel config"
rm -rf "$BUILD"
make -C "$SRC" ARCH=x86_64 mrproper >/dev/null
make -C "$SRC" O="$BUILD" ARCH=x86_64 tinyconfig >/dev/null
make -C "$SRC" O="$BUILD" ARCH=x86_64 rustavailable >/dev/null

CONFIG="$BUILD/.config"
CONFIG_TOOL="$SRC/scripts/config"

for symbol in   CONFIG_RUST   CONFIG_ACPI   CONFIG_RELOCATABLE   CONFIG_DEVTMPFS   CONFIG_DEVTMPFS_MOUNT   CONFIG_PROC_FS   CONFIG_SYSFS   CONFIG_TMPFS   CONFIG_BINFMT_ELF   CONFIG_TTY   CONFIG_SERIAL_8250   CONFIG_SERIAL_8250_CONSOLE   CONFIG_SERIAL_CORE   CONFIG_SERIAL_CORE_CONSOLE   CONFIG_PRINTK   CONFIG_NET   CONFIG_PCI   CONFIG_DRM   CONFIG_DRM_BOCHS   CONFIG_INPUT_EVDEV   CONFIG_MULTIUSER   CONFIG_BLOCK   CONFIG_PARTITION_ADVANCED   CONFIG_EFI_PARTITION   CONFIG_ATA   CONFIG_ATA_PIIX   CONFIG_SATA_AHCI   CONFIG_SCSI   CONFIG_BLK_DEV_SD   CONFIG_BLK_DEV_LOOP   CONFIG_EXT4_FS   CONFIG_BTRFS_FS   CONFIG_MISC_FILESYSTEMS   CONFIG_SQUASHFS   CONFIG_SQUASHFS_ZLIB   CONFIG_SQUASHFS_ZSTD   CONFIG_SHMEM   CONFIG_UNIX   CONFIG_EPOLL   CONFIG_SIGNALFD   CONFIG_EVENTFD   CONFIG_TIMERFD   CONFIG_NAMESPACES   CONFIG_PID_NS   CONFIG_MOUNT_NS; do
  "$CONFIG_TOOL" --file "$CONFIG" --enable "$symbol"
done
"$CONFIG_TOOL" --file "$CONFIG" --set-str CONFIG_CMDLINE "console=ttyS0"
"$CONFIG_TOOL" --file "$CONFIG" --set-str CONFIG_SYSTEM_TRUSTED_KEYS ""
"$CONFIG_TOOL" --file "$CONFIG" --set-str CONFIG_SYSTEM_REVOCATION_KEYS ""

make -C "$SRC" O="$BUILD" ARCH=x86_64 olddefconfig >/dev/null

grep -q '^CONFIG_RUST=y$' "$CONFIG"
grep -q '^CONFIG_RELOCATABLE=y$' "$CONFIG"
grep -q '^CONFIG_BLOCK=y$' "$CONFIG"
grep -q '^CONFIG_ATA=y$' "$CONFIG"
grep -q '^CONFIG_SATA_AHCI=y$' "$CONFIG"
grep -q '^CONFIG_EXT4_FS=y$' "$CONFIG"
grep -q '^CONFIG_BTRFS_FS=y$' "$CONFIG"
grep -q '^CONFIG_DRM=y$' "$CONFIG"
grep -q '^CONFIG_DRM_BOCHS=y$' "$CONFIG"
grep -q '^CONFIG_INPUT_EVDEV=y$' "$CONFIG"
grep -q '^CONFIG_MULTIUSER=y$' "$CONFIG"
grep -q '^CONFIG_SQUASHFS=y$' "$CONFIG"
grep -q '^CONFIG_BLK_DEV_LOOP=y$' "$CONFIG"
if grep -q '^CONFIG_INITRAMFS_SOURCE=' "$CONFIG"; then
  echo "real-init kernel unexpectedly contains CONFIG_INITRAMFS_SOURCE" >&2
  exit 1
fi

echo "[4/5] Building direct-init storage bzImage"
make -C "$SRC" O="$BUILD" ARCH=x86_64 -j"$JOBS" bzImage >/dev/null

echo "[5/5] Publishing artifacts"
install -Dm644 "$BUILD/arch/x86/boot/bzImage" "$ROOT/out/luna-real-init.efi"
install -Dm644 "$CONFIG" "$ROOT/out/luna-real-init.config"
install -Dm755 "$LUNA_INIT" "$ROOT/out/luna-real-init.init"
file "$ROOT/out/luna-real-init.efi"
echo "Built: $ROOT/out/luna-real-init.efi"
echo "Init:  $ROOT/out/luna-real-init.init"
