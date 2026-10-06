#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${ROOT}/out"
KERNEL_VERSION="${LUNA_EXPERIMENT_KERNEL_VERSION:-7.2.4}"
KERNEL_TARBALL="${OUT}/linux-${KERNEL_VERSION}.tar.xz"
KERNEL_SRC="${OUT}/linux-${KERNEL_VERSION}"
KERNEL_URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${KERNEL_VERSION}.tar.xz"
ROOTFS="${OUT}/rootfs"
INITRAMFS_LIST="${OUT}/initramfs.list"
INIT="${ROOTFS}/init"
JOBS="${LUNA_EXPERIMENT_JOBS:-$(nproc)}"

mkdir -p "$OUT" "$ROOTFS/bin"

for tool in curl tar cargo make; do
    command -v "$tool" >/dev/null 2>&1 || { echo "missing required tool: $tool" >&2; exit 1; }
done

command -v busybox >/dev/null 2>&1 || {
    echo "missing required bootstrap tool: busybox" >&2
    exit 1
}

printf '[1/4] Building luna-init (musl)\n'
if ! rustup target list --installed | grep -qx 'x86_64-unknown-linux-musl'; then
    echo 'Rust musl target is not installed; attempting to install it.'
    rustup target add x86_64-unknown-linux-musl
fi
cargo build --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-unknown-linux-musl
install -Dm755 "$ROOT/target/x86_64-unknown-linux-musl/release/luna-experimental-init" "$INIT"

# BusyBox is temporary Stage 0 bootstrap material only. It provides /bin/sh
# and sleep so PID 1 can prove a stable boot without a service manager.
install -Dm755 "$(command -v busybox)" "$ROOTFS/bin/busybox"
ln -sfn /bin/busybox "$ROOTFS/bin/sh"

printf '[2/4] Preparing Linux kernel\n'
if [ ! -f "$KERNEL_TARBALL" ]; then
    curl --fail --location --retry 3 --output "$KERNEL_TARBALL" "$KERNEL_URL"
fi
if [ ! -d "$KERNEL_SRC" ]; then
    tar -xJf "$KERNEL_TARBALL" -C "$OUT"
fi

cd "$KERNEL_SRC"
make mrproper
make O="$KERNEL_SRC/build" ARCH=x86_64 x86_64_defconfig

# Stage 0 needs only EFI loading, initramfs execution, a console, and the
# minimal pseudo-filesystems required by a normal Linux PID 1.
./scripts/config --file "$KERNEL_SRC/build/.config" \
    -e CONFIG_EFI \
    -e CONFIG_EFI_STUB \
    -e CONFIG_EFI_MIXED \
    -e CONFIG_BLK_DEV_INITRD \
    -e CONFIG_DEVTMPFS \
    -e CONFIG_DEVTMPFS_MOUNT \
    -e CONFIG_PROC_FS \
    -e CONFIG_SYSFS \
    -e CONFIG_TMPFS \
    -e CONFIG_SHMEM \
    -e CONFIG_BINFMT_ELF \
    -e CONFIG_TTY \
    -e CONFIG_SERIAL_8250 \
    -e CONFIG_SERIAL_8250_CONSOLE \
    -e CONFIG_SERIAL_CORE \
    -e CONFIG_SERIAL_CORE_CONSOLE

printf '[3/4] Building embedded initramfs\n'
cat > "$INITRAMFS_LIST" <<EOF
dir /dev 0755 0 0
dir /proc 0555 0 0
dir /sys 0555 0 0
dir /tmp 1777 0 0
file /init $INIT 0755 0 0
file /bin/busybox $ROOTFS/bin/busybox 0755 0 0
slink /bin/sh /bin/busybox 0777 0 0
EOF

./usr/gen_initramfs_list.sh "$INITRAMFS_LIST" > "$OUT/initramfs.cpio"
./scripts/config --file "$KERNEL_SRC/build/.config" --set-str CONFIG_INITRAMFS_SOURCE "$OUT/initramfs.cpio"
make O="$KERNEL_SRC/build" ARCH=x86_64 olddefconfig

printf '[4/4] Building one-file EFI kernel\n'
make O="$KERNEL_SRC/build" ARCH=x86_64 -j"$JOBS" bzImage
install -Dm644 "$KERNEL_SRC/build/arch/x86/boot/bzImage" "$OUT/luna-onefile.efi"

printf '\nBuilt: %s\n' "$OUT/luna-onefile.efi"
printf 'QEMU: qemu-system-x86_64 -nographic -kernel %s\n' "$OUT/luna-onefile.efi"
