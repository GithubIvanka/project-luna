#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
OUT="${REPO_ROOT}/boot/luna-boot/tests/ovmf/out"
SYSROOT="${OUT}/system-root"
INITROOT="${OUT}/initramfs-root"

: "${BUSYBOX:?Set BUSYBOX to a static x86_64 BusyBox binary}"
: "${LUNA_TEST_KERNEL:?Set LUNA_TEST_KERNEL to a Linux x86_64 bzImage}"

for tool in cargo rustup mksquashfs cpio gzip file; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
[ -x "$BUSYBOX" ] || { echo "BUSYBOX is not executable: $BUSYBOX" >&2; exit 1; }

RUNTIME_TARGET="x86_64-unknown-linux-musl"
if ! rustup target list --installed | grep -qx "$RUNTIME_TARGET"; then
    echo "luna-test userspace: installing Rust target $RUNTIME_TARGET" >&2
    rustup target add "$RUNTIME_TARGET"
fi

rm -rf "$SYSROOT" "$INITROOT"
mkdir -p "$SYSROOT"/{bin,sbin,dev,proc,sys,run,tmp,etc,data} "$INITROOT"/{bin,dev,proc,sys,run,newroot}

cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
cargo build --release --manifest-path "$REPO_ROOT/components/luna-init/Cargo.toml" --target "$RUNTIME_TARGET"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/luna-init/target/$RUNTIME_TARGET/release/luna-init"
if [ ! -x "$LUNA_INIT" ]; then
    LUNA_INIT="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-init"
fi
[ -x "$RUNTIME" ] || { echo "system runtime binary was not produced: $RUNTIME" >&2; exit 1; }
[ -x "$LUNA_INIT" ] || { echo "luna-init binary was not produced: $LUNA_INIT" >&2; exit 1; }
if file "$RUNTIME" | grep -qv 'statically linked'; then
    echo "luna-test userspace: system-runtime must be statically linked with musl" >&2
    exit 1
fi
if file "$LUNA_INIT" | grep -qv 'statically linked'; then
    echo "luna-test userspace: luna-init must be statically linked with musl" >&2
    exit 1
fi

cp "$BUSYBOX" "$SYSROOT/bin/busybox"
chmod 0755 "$SYSROOT/bin/busybox"
for applet in sh ls cat mount umount ps pwd echo; do
    ln -sf busybox "$SYSROOT/bin/$applet"
done

cp "$RUNTIME" "$SYSROOT/sbin/luna-system-runtime"
chmod 0755 "$SYSROOT/sbin/luna-system-runtime"
ln -sf luna-system-runtime "$SYSROOT/sbin/init"

mkdir -p "$SYSROOT"/{boot,home,lib,lib64,media,mnt,opt,root,srv,usr,var}
mkdir -p "$SYSROOT/usr/bin" "$SYSROOT/usr/lib" "$SYSROOT/usr/sbin" "$SYSROOT/etc/luna"

mksquashfs "$SYSROOT" "$OUT/luna-test.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

cp "$BUSYBOX" "$INITROOT/bin/busybox"
chmod 0755 "$INITROOT/bin/busybox"
cp "$LUNA_INIT" "$INITROOT/init"
chmod 0755 "$INITROOT/init"

(
    cd "$INITROOT"
    find . -print0 | cpio --null -o -H newc --quiet | gzip -9 > "$OUT/luna-test-initramfs.img"
)

echo "Built Luna QEMU userspace:"
echo "  System Image: $OUT/luna-test.squashfs"
echo "  initramfs:    $OUT/luna-test-initramfs.img"
echo "  runtime:      $RUNTIME"
echo "  luna-init:    $LUNA_INIT"
echo "  kernel:       $LUNA_TEST_KERNEL"
