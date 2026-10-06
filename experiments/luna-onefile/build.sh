#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${ROOT}/out"
KERNEL_VERSION="${LUNA_EXPERIMENT_KERNEL_VERSION:-7.2.4}"
KERNEL_TARBALL="${OUT}/linux-${KERNEL_VERSION}.tar.xz"
KERNEL_SRC="${OUT}/linux-${KERNEL_VERSION}"
KERNEL_URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${KERNEL_VERSION}.tar.xz"
KERNEL_BUILD="${ROOT}/kernel/build"
KERNEL_CONFIG="${LUNA_EXPERIMENT_KERNEL_CONFIG:-${ROOT}/kernel/minimal-x86_64.config}"
ROOTFS="${OUT}/rootfs"
INITRAMFS_LIST="${OUT}/initramfs.list"
INIT="${ROOTFS}/init"
JOBS="${LUNA_EXPERIMENT_JOBS:-$(nproc)}"

mkdir -p "$OUT" "$ROOTFS" "$(dirname "$KERNEL_BUILD")"

for tool in curl tar cargo make cc rustup; do
    command -v "$tool" >/dev/null 2>&1 || { echo "missing required tool: $tool" >&2; exit 1; }
done

[ -f "$KERNEL_CONFIG" ] || {
    echo "missing kernel config fragment: $KERNEL_CONFIG" >&2
    exit 1
}

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

printf '[2/4] Preparing minimal Linux kernel\n'
if [ ! -f "$KERNEL_TARBALL" ]; then
    curl --fail --location --retry 3 --output "$KERNEL_TARBALL" "$KERNEL_URL"
fi
if [ ! -d "$KERNEL_SRC" ]; then
    tar -xJf "$KERNEL_TARBALL" -C "$OUT"
fi

rm -rf "$KERNEL_BUILD"
cd "$KERNEL_SRC"
make ARCH=x86_64 mrproper
make O="$KERNEL_BUILD" ARCH=x86_64 tinyconfig

# Apply the deliberately small Luna Stage 0.1 fragment without invoking
# merge_config.sh. This keeps Kbuild's source-tree state deterministic.
"$KERNEL_SRC/scripts/config" --file "$KERNEL_BUILD/.config" \
    --enable CONFIG_ACPI \
    --enable CONFIG_EFI \
    --enable CONFIG_EFI_STUB \
    --enable CONFIG_BLK_DEV_INITRD \
    --enable CONFIG_DEVTMPFS \
    --enable CONFIG_DEVTMPFS_MOUNT \
    --enable CONFIG_PROC_FS \
    --enable CONFIG_SYSFS \
    --enable CONFIG_TMPFS \
    --enable CONFIG_BINFMT_ELF \
    --enable CONFIG_TTY \
    --enable CONFIG_VT \
    --enable CONFIG_VT_CONSOLE \
    --enable CONFIG_SERIAL_8250 \
    --enable CONFIG_SERIAL_8250_CONSOLE \
    --enable CONFIG_SERIAL_CORE \
    --enable CONFIG_SERIAL_CORE_CONSOLE \
    --enable CONFIG_PRINTK \
    --enable CONFIG_PRINTK_TIME \
    --enable CONFIG_EARLY_PRINTK \
    --enable CONFIG_CMDLINE_BOOL \
    --set-str CONFIG_CMDLINE "console=ttyS0 rdinit=/init"

make O="$KERNEL_BUILD" ARCH=x86_64 olddefconfig

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

PATH="$KERNEL_SRC/usr:$PATH" "$KERNEL_SRC/usr/gen_initramfs.sh" "$INITRAMFS_LIST" > "$OUT/initramfs.cpio"
"$KERNEL_SRC/scripts/config" --file "$KERNEL_BUILD/.config" --set-str CONFIG_INITRAMFS_SOURCE "$OUT/initramfs.cpio"
make O="$KERNEL_BUILD" ARCH=x86_64 olddefconfig

printf '[4/4] Building one-file EFI kernel\n'
make O="$KERNEL_BUILD" ARCH=x86_64 -j"$JOBS" bzImage
install -Dm644 "$KERNEL_BUILD/arch/x86/boot/bzImage" "$OUT/luna-onefile.efi"

printf '\nBuilt: %s\n' "$OUT/luna-onefile.efi"
printf 'Kernel config: %s\n' "$KERNEL_CONFIG"
printf 'QEMU: qemu-system-x86_64 -nographic -kernel %s\n' "$OUT/luna-onefile.efi"
