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

mkdir -p "$OUT" "$ROOTFS"

for tool in curl tar cargo make cc rustup; do
    command -v "$tool" >/dev/null 2>&1 || { echo "missing required tool: $tool" >&2; exit 1; }
done

printf '[1/4] Building luna-init (Rust no_std)\n'
if ! rustup target list --installed | grep -qx 'x86_64-unknown-linux-musl'; then
    echo 'Rust musl target is not installed; attempting to install it.'
    rustup target add x86_64-unknown-linux-musl
fi
RUSTFLAGS="${RUSTFLAGS:-} -C relocation-model=static" \
    cargo build --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-unknown-linux-musl
cc -nostdlib -static -fno-pie -no-pie -Wl,-e,_start -O2 -s \
    -o "$INIT" \
    "$ROOT/start.c" \
    "$ROOT/target/x86_64-unknown-linux-musl/release/libluna_experimental_init.a"
file "$INIT"

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
    -e CONFIG_SERIAL_CORE_CONSOLE \
    -e CONFIG_CMDLINE_BOOL \
    --set-str CONFIG_CMDLINE "console=ttyS0 rdinit=/init"

make O="$KERNEL_SRC/build" ARCH=x86_64 olddefconfig
cc -O2 -o "$KERNEL_SRC/usr/gen_init_cpio" "$KERNEL_SRC/usr/gen_init_cpio.c"

printf '[3/4] Building embedded initramfs\n'
cat > "$INITRAMFS_LIST" <<EOF
dir /dev 0755 0 0
dir /proc 0555 0 0
dir /sys 0555 0 0
dir /tmp 1777 0 0
nod /dev/console 0600 0 0 c 5 1
nod /dev/null 0666 0 0 c 1 3
file /init $INIT 0755 0 0
EOF

PATH="$KERNEL_SRC/usr:$PATH" ./usr/gen_initramfs.sh "$INITRAMFS_LIST" > "$OUT/initramfs.cpio"
./scripts/config --file "$KERNEL_SRC/build/.config" --set-str CONFIG_INITRAMFS_SOURCE "$OUT/initramfs.cpio"
make O="$KERNEL_SRC/build" ARCH=x86_64 olddefconfig

printf '[4/4] Building one-file EFI kernel\n'
make O="$KERNEL_SRC/build" ARCH=x86_64 -j"$JOBS" bzImage
install -Dm644 "$KERNEL_SRC/build/arch/x86/boot/bzImage" "$OUT/luna-onefile.efi"

printf '\nBuilt: %s\n' "$OUT/luna-onefile.efi"
printf 'QEMU: qemu-system-x86_64 -nographic -kernel %s -append "console=ttyS0 rdinit=/init"\n' "$OUT/luna-onefile.efi"
