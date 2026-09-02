#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
WORK="${OUT}/work"
SYSTEM_ROOT="${WORK}/system-root"
INIT_ROOT="${WORK}/initramfs-root"
DATA_ROOT="${WORK}/data-root"

LUNA_VERSION="${LUNA_VERSION:-0.1.0}"
KERNEL="${LUNA_TEST_KERNEL:-}"
BUSYBOX="${BUSYBOX:-}"
DESKTOP_ROOT="${LUNA_DESKTOP_ROOT:-}"

find_latest() {
    local pattern="$1"
    local value
    value=$(compgen -G "$pattern" | sort -V | tail -n 1 || true)
    [ -n "$value" ] && printf '%s\n' "$value"
}

if [ -z "$KERNEL" ]; then KERNEL="$(find_latest '/boot/vmlinuz-*' || true)"; fi
if [ -z "$BUSYBOX" ]; then
    for candidate in /usr/bin/busybox /bin/busybox; do
        if [ -x "$candidate" ]; then BUSYBOX="$candidate"; break; fi
    done
fi

: "${KERNEL:?No Linux kernel found. Set LUNA_TEST_KERNEL to an x86_64 vmlinuz.}"
: "${BUSYBOX:?No BusyBox found. Set BUSYBOX to a static x86_64 BusyBox binary.}"
: "${DESKTOP_ROOT:?Luna PC images require a prepared graphical desktop root. Set LUNA_DESKTOP_ROOT to the final System Image root containing luna-login and niri-session.}"

for tool in cargo rustup sgdisk mkfs.ext4 mkfs.fat mcopy mmd dd mksquashfs cpio gzip file; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
[ -f "$KERNEL" ] || { echo "kernel not found: $KERNEL" >&2; exit 1; }
[ -x "$BUSYBOX" ] || { echo "BusyBox is not executable: $BUSYBOX" >&2; exit 1; }
[ -d "$DESKTOP_ROOT" ] || { echo "LUNA_DESKTOP_ROOT is not a directory: $DESKTOP_ROOT" >&2; exit 1; }
[ -x "$DESKTOP_ROOT/usr/bin/luna-login" ] || { echo "graphical login missing: $DESKTOP_ROOT/usr/bin/luna-login" >&2; exit 1; }
[ -x "$DESKTOP_ROOT/usr/bin/niri-session" ] || { echo "niri session missing: $DESKTOP_ROOT/usr/bin/niri-session" >&2; exit 1; }

RUNTIME_TARGET="x86_64-unknown-linux-musl"
if ! rustup target list --installed | grep -qx "$RUNTIME_TARGET"; then rustup target add "$RUNTIME_TARGET"; fi
if ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1 && ! command -v musl-gcc >/dev/null 2>&1; then
    echo "A musl C linker is required (install musl-tools/musl-gcc)." >&2; exit 1
fi

KERNEL_BASENAME="$(basename "$KERNEL")"
KERNEL_VERSION="${LUNA_KERNEL_VERSION:-${KERNEL_BASENAME#vmlinuz-}}"
[ -n "$KERNEL_VERSION" ] || { echo "could not determine kernel version" >&2; exit 1; }

mkdir -p "$OUT" "$WORK"
rm -rf "$SYSTEM_ROOT" "$INIT_ROOT" "$DATA_ROOT" "$WORK/system-partition"
rm -f "$OUT"/luna-pc.img "$OUT"/luna-efi.img "$OUT"/luna-system.img "$OUT"/luna-data.img "$OUT"/luna-initramfs.img "$OUT"/luna-${LUNA_VERSION}.squashfs "$OUT"/BUILD-INFO "$OUT"/SHA256SUMS

cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
[ -x "$RUNTIME" ] || { echo "runtime binary was not produced: $RUNTIME" >&2; exit 1; }
file "$RUNTIME" | grep -q 'statically linked' || { echo "luna-system-runtime is not statically linked; refusing to build PC image" >&2; exit 1; }

cargo build --release -p luna-login
LOGIN="$REPO_ROOT/target/release/luna-login"
[ -x "$LOGIN" ] || { echo "luna-login was not produced: $LOGIN" >&2; exit 1; }

cargo build --release --target x86_64-unknown-uefi --manifest-path "$REPO_ROOT/boot/luna-boot/Cargo.toml"
EFI="$REPO_ROOT/boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi"
[ -f "$EFI" ] || { echo "UEFI loader was not produced: $EFI" >&2; exit 1; }

mkdir -p "$SYSTEM_ROOT"/{bin,sbin,etc,dev,proc,sys,run,tmp,boot,home,lib,lib64,media,mnt,opt,root,srv,usr,var,data}
mkdir -p "$SYSTEM_ROOT/usr/bin" "$SYSTEM_ROOT/usr/sbin" "$SYSTEM_ROOT/usr/lib" "$SYSTEM_ROOT/etc/luna" "$SYSTEM_ROOT/etc/pam.d"
cp "$BUSYBOX" "$SYSTEM_ROOT/bin/busybox"; chmod 0755 "$SYSTEM_ROOT/bin/busybox"
for applet in sh ls cat mount umount ps pwd echo clear hostname dmesg mkdir rm; do ln -sf busybox "$SYSTEM_ROOT/bin/$applet"; done
cp "$RUNTIME" "$SYSTEM_ROOT/sbin/luna-system-runtime"; chmod 0755 "$SYSTEM_ROOT/sbin/luna-system-runtime"; ln -sf luna-system-runtime "$SYSTEM_ROOT/sbin/init"

cat > "$SYSTEM_ROOT/etc/os-release" <<EOF
NAME="Project Luna"
ID=luna
VERSION="$LUNA_VERSION"
PRETTY_NAME="Project Luna $LUNA_VERSION"
EOF
cat > "$SYSTEM_ROOT/etc/hostname" <<'EOF'
luna
EOF
cat > "$SYSTEM_ROOT/etc/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
luna:x:1000:1000:Luna User:/home/luna:/usr/bin/fish
EOF
cat > "$SYSTEM_ROOT/etc/group" <<'EOF'
root:x:0:
luna:x:1000:
EOF
cat > "$SYSTEM_ROOT/etc/profile" <<'EOF'
export PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
export HOME=/home/luna
export TERM=${TERM:-linux}
export SHELL=/usr/bin/fish
EOF

cp -a "$DESKTOP_ROOT"/. "$SYSTEM_ROOT"/

# The desktop payload is responsible for graphical applications. SYSTEM owns
# only the final identity database entries that are specific to this image.
if ! grep -q '^luna:' "$SYSTEM_ROOT/etc/passwd"; then
    printf '%s\n' 'luna:x:1000:1000:Luna User:/home/luna:/usr/bin/fish' >> "$SYSTEM_ROOT/etc/passwd"
fi
if ! grep -q '^root:' "$SYSTEM_ROOT/etc/passwd"; then
    printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' >> "$SYSTEM_ROOT/etc/passwd"
fi
if ! grep -q '^greeter:' "$SYSTEM_ROOT/etc/passwd"; then
    printf '%s\n' 'greeter:x:992:992:Luna Graphical Login:/var/lib/noctalia-greeter:/usr/bin/sh' >> "$SYSTEM_ROOT/etc/passwd"
fi
if ! grep -q '^greeter:' "$SYSTEM_ROOT/etc/group"; then
    printf '%s\n' 'greeter:x:992:' >> "$SYSTEM_ROOT/etc/group"
fi
mkdir -p "$SYSTEM_ROOT/var/lib/noctalia-greeter" "$SYSTEM_ROOT/home/luna"
chown 992:992 "$SYSTEM_ROOT/var/lib/noctalia-greeter"
chown 1000:1000 "$SYSTEM_ROOT/home/luna"
chmod 0700 "$SYSTEM_ROOT/home/luna"

cat > "$SYSTEM_ROOT/etc/luna/graphical-login" <<'EOF'
/usr/bin/luna-login
EOF
cat > "$SYSTEM_ROOT/etc/luna/graphical-session" <<'EOF'
/usr/bin/luna-run-session
EOF
printf 'graphical\n' > "$SYSTEM_ROOT/etc/luna/mode"

mksquashfs "$SYSTEM_ROOT" "$OUT/luna-${LUNA_VERSION}.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

mkdir -p "$INIT_ROOT"/{bin,dev,proc,sys,run,newroot}
cp "$BUSYBOX" "$INIT_ROOT/bin/busybox"; chmod 0755 "$INIT_ROOT/bin/busybox"
cp "$REPO_ROOT/boot/luna-boot/tests/ovmf/luna-init" "$INIT_ROOT/init"; chmod 0755 "$INIT_ROOT/init"
(
    cd "$INIT_ROOT"
    find . -print0 | cpio --null -o -H newc --quiet | gzip -9 > "$OUT/luna-initramfs.img"
)

mkdir -p "$DATA_ROOT/system/apps" "$DATA_ROOT/system/drivers" "$DATA_ROOT/system/libs" "$DATA_ROOT/system/volumes" "$DATA_ROOT/system/config" "$DATA_ROOT/system/state" "$DATA_ROOT/users/luna/home" "$DATA_ROOT/users/luna/data" "$DATA_ROOT/users/luna/config" "$DATA_ROOT/cache"
mkfs.ext4 -q -F -L LUNA-DATA -d "$DATA_ROOT" "$OUT/luna-data.img" 512M

mkdir -p "$WORK/system-partition/images" "$WORK/system-partition/kernels/$KERNEL_VERSION"
cp "$OUT/luna-${LUNA_VERSION}.squashfs" "$WORK/system-partition/images/luna-${LUNA_VERSION}.squashfs"
cp "$OUT/luna-initramfs.img" "$WORK/system-partition/kernels/$KERNEL_VERSION/initramfs.img"
cp "$KERNEL" "$WORK/system-partition/kernels/$KERNEL_VERSION/bzImage"
cat > "$WORK/system-partition/images/luna-${LUNA_VERSION}.toml" <<EOF
[image]
name = "luna"
version = "$LUNA_VERSION"
format = "squashfs"

[architecture]
arch = "x86_64"

[kernels]
compatible = ["$KERNEL_VERSION"]
EOF
mkfs.ext4 -q -F -L LUNA-SYSTEM -d "$WORK/system-partition" "$OUT/luna-system.img" 384M

truncate -s 1152M "$OUT/luna-pc.img"
sgdisk --zap-all "$OUT/luna-pc.img" >/dev/null
sgdisk -n 1:2048:+128M -t 1:ef00 -c 1:EFI -n 2:264192:+384M -t 2:8300 -c 2:SYSTEM -n 3:1056768:+512M -t 3:8300 -c 3:DATA "$OUT/luna-pc.img" >/dev/null
truncate -s 128M "$OUT/luna-efi.img"
mkfs.fat -F 32 "$OUT/luna-efi.img" >/dev/null
mmd -i "$OUT/luna-efi.img" ::/EFI; mmd -i "$OUT/luna-efi.img" ::/EFI/LUNA; mmd -i "$OUT/luna-efi.img" ::/EFI/BOOT
mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI; mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
dd if="$OUT/luna-efi.img" of="$OUT/luna-pc.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/luna-system.img" of="$OUT/luna-pc.img" bs=512 seek=264192 conv=notrunc status=none
dd if="$OUT/luna-data.img" of="$OUT/luna-pc.img" bs=512 seek=1056768 conv=notrunc status=none

cat > "$OUT/BUILD-INFO" <<EOF
Project Luna PC graphical development image
version=$LUNA_VERSION
architecture=x86_64
system_image=luna-${LUNA_VERSION}.squashfs
system_libc=musl
bootloader=luna-boot.efi
uefi_fallback=EFI/BOOT/BOOTX64.EFI
kernel_version=$KERNEL_VERSION
partitions=EFI:128MiB,SYSTEM:384MiB,DATA:512MiB
image_size=1152MiB
boot_ui=graphical
login_ui=/usr/bin/luna-login
desktop=/usr/bin/niri-session
shell=/usr/bin/fish
terminal=/usr/bin/ghostty
compatibility_shells=/usr/bin/bash,/usr/bin/sh
verbose_boot=boot-menu-only
EOF
sha256sum "$OUT/luna-pc.img" "$OUT/luna-${LUNA_VERSION}.squashfs" "$OUT/luna-initramfs.img" > "$OUT/SHA256SUMS"

echo "Built Project Luna graphical PC image: $OUT/luna-pc.img"
