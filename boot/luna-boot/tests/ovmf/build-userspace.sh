#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
OUT="${REPO_ROOT}/boot/luna-boot/tests/ovmf/out"
SYSROOT="${OUT}/system-root"
INITROOT="${OUT}/initramfs-root"

: "${BUSYBOX:?Set BUSYBOX to a static x86_64 BusyBox binary}"
: "${LUNA_TEST_KERNEL:?Set LUNA_TEST_KERNEL to a Linux x86_64 bzImage}"

for tool in cargo mksquashfs cpio gzip ldd; do command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }; done
[ -x "$BUSYBOX" ] || { echo "BUSYBOX is not executable: $BUSYBOX" >&2; exit 1; }

rm -rf "$SYSROOT" "$INITROOT"
mkdir -p "$SYSROOT"/{bin,sbin,dev,proc,sys,run,tmp,etc,data} "$INITROOT"/{bin,dev,proc,sys,run,newroot}

cargo build --release -p luna-system-runtime
RUNTIME="$REPO_ROOT/target/release/luna-system-runtime"
[ -x "$RUNTIME" ] || { echo "runtime binary was not produced: $RUNTIME" >&2; exit 1; }

cp "$BUSYBOX" "$SYSROOT/bin/busybox"
chmod 0755 "$SYSROOT/bin/busybox"
ln -sf busybox "$SYSROOT/bin/sh"
ln -sf busybox "$SYSROOT/bin/ls"
ln -sf busybox "$SYSROOT/bin/cat"
ln -sf busybox "$SYSROOT/bin/mount"
ln -sf busybox "$SYSROOT/bin/umount"
ln -sf busybox "$SYSROOT/bin/ps"
ln -sf busybox "$SYSROOT/bin/pwd"
ln -sf busybox "$SYSROOT/bin/echo"
cp "$RUNTIME" "$SYSROOT/sbin/luna-system-runtime"
chmod 0755 "$SYSROOT/sbin/luna-system-runtime"
ln -sf luna-system-runtime "$SYSROOT/sbin/init"

# The runtime is a normal host Linux executable. Copy its dynamic loader and
# shared libraries into the test System Image so the image is self-contained.
while IFS= read -r lib; do
    [ -n "$lib" ] || continue
    [ -e "$lib" ] || continue
    destination="$SYSROOT$lib"
    mkdir -p "$(dirname "$destination")"
    cp -L "$lib" "$destination"
done < <(ldd "$RUNTIME" | awk '{for (i=1; i<=NF; i++) if ($i ~ /^\//) {print $i; break}}' | sort -u)

# Give the first userspace a conventional root hierarchy.
mkdir -p "$SYSROOT"/{boot,home,lib,lib64,media,mnt,opt,root,srv,usr,var}
mkdir -p "$SYSROOT/usr/bin" "$SYSROOT/usr/lib" "$SYSROOT/usr/sbin" "$SYSROOT/etc/luna"

mksquashfs "$SYSROOT" "$OUT/luna-test.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

cp "$BUSYBOX" "$INITROOT/bin/busybox"
chmod 0755 "$INITROOT/bin/busybox"
cat > "$INITROOT/init" <<'EOF'
#!/bin/busybox sh
set -eu

BB=/bin/busybox

$BB mount -t proc proc /proc
$BB mount -t sysfs sysfs /sys
$BB mount -t devtmpfs devtmpfs /dev 2>/dev/null || $BB mount -t tmpfs -o mode=0755,nosuid devtmpfs /dev
$BB mkdir -p /run/system /newroot

SYSTEM_IMAGE=/images/luna-test.squashfs
for arg in $(cat /proc/cmdline); do
    case "$arg" in
        luna.system_image=*) SYSTEM_IMAGE="${arg#luna.system_image=}" ;;
    esac
done

$BB mount -t ext4 -o ro /dev/sda2 /run/system
[ -f "/run/system${SYSTEM_IMAGE}" ] || { echo "Luna init: system image not found: ${SYSTEM_IMAGE}" >&2; exec $BB sh; }
$BB mount -t squashfs -o ro,loop "/run/system${SYSTEM_IMAGE}" /newroot
$BB mkdir -p /newroot/data /newroot/dev /newroot/proc /newroot/sys /newroot/run
$BB mount -t ext4 /dev/sda3 /newroot/data
$BB mount --move /dev /newroot/dev
$BB mount --move /proc /newroot/proc
$BB mount --move /sys /newroot/sys
$BB mount --move /run/system /newroot/run/system

exec $BB switch_root -c /dev/console /newroot /sbin/init
EOF
chmod 0755 "$INITROOT/init"

# The kernel initramfs format is a newc CPIO archive. Keeping this builder
# separate from luna-boot makes the early-userspace handoff reproducible.
(
    cd "$INITROOT"
    find . -print0 | cpio --null -o -H newc --quiet | gzip -9 > "$OUT/luna-test-initramfs.img"
)

cp "$OUT/luna-test.squashfs" "$OUT/luna-test.squashfs.ready"
cp "$OUT/luna-test-initramfs.img" "$OUT/luna-test-initramfs.img.ready"

echo "Built:"
echo "  $OUT/luna-test.squashfs"
echo "  $OUT/luna-test-initramfs.img"
echo "  kernel: $LUNA_TEST_KERNEL"
