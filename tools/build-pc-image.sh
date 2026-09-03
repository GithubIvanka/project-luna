#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
WORK="${OUT}/work"
SYSTEM_ROOT="${WORK}/system-root"
INIT_ROOT="${WORK}/initramfs-root"
DATA_ROOT="${WORK}/data-root"

LUNA_VERSION="${LUNA_VERSION:-0.1.0}"
LUNA_PASSWORD_HASH="${LUNA_PASSWORD_HASH:-}"
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
mkdir -p "$SYSTEM_ROOT/usr/bin" "$SYSTEM_ROOT/usr/sbin" "$SYSTEM_ROOT/usr/lib" "$SYSTEM_ROOT/etc/luna" "$SYSTEM_ROOT/etc/pam.d" "$SYSTEM_ROOT/usr/share"
cp "$BUSYBOX" "$SYSTEM_ROOT/bin/busybox"; chmod 0755 "$SYSTEM_ROOT/bin/busybox"
for applet in sh ls cat mount umount ps pwd echo clear hostname dmesg mkdir rm chmod id; do ln -sf busybox "$SYSTEM_ROOT/bin/$applet"; done
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
greeter:x:992:992:Luna Graphical Login:/var/lib/noctalia-greeter:/usr/bin/sh
EOF
cat > "$SYSTEM_ROOT/etc/group" <<'EOF'
root:x:0:
luna:x:1000:
greeter:x:992:
EOF
cat > "$SYSTEM_ROOT/etc/shadow" <<EOF
root:!:1:0:99999:7:::
luna:${LUNA_PASSWORD_HASH:-!:1:0:99999:7:::}
greeter:!:1:0:99999:7:::
EOF
chmod 0640 "$SYSTEM_ROOT/etc/shadow"
cat > "$SYSTEM_ROOT/etc/profile" <<'EOF'
export PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
export HOME=/home/luna
export TERM=${TERM:-linux}
export SHELL=/usr/bin/fish
EOF

cp -a "$DESKTOP_ROOT"/. "$SYSTEM_ROOT"/

rm -rf "$SYSTEM_ROOT/home"
mkdir -p "$SYSTEM_ROOT/data/users/luna/home" "$SYSTEM_ROOT/data/users/luna/data" "$SYSTEM_ROOT/data/users/luna/config" "$SYSTEM_ROOT/data/cache"
ln -s /data/users/luna/home "$SYSTEM_ROOT/home"
mkdir -p "$SYSTEM_ROOT/var/lib/noctalia-greeter"
chown 1000:1000 "$SYSTEM_ROOT/data/users/luna/home" "$SYSTEM_ROOT/data/users/luna/data" "$SYSTEM_ROOT/data/users/luna/config"
chown 992:992 "$SYSTEM_ROOT/var/lib/noctalia-greeter"
chmod 0700 "$SYSTEM_ROOT/data/users/luna/home"

cat > "$SYSTEM_ROOT/etc/luna/graphical-login" <<'EOF'
/usr/bin/luna-login
EOF
cat > "$SYSTEM_ROOT/etc/luna/graphical-session" <<'EOF'
/usr/bin/niri-session
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

SYSTEM_SIZE_MIB="${LUNA_SYSTEM_SIZE_MIB:-512}"
DATA_SIZE_MIB="${LUNA_DATA_SIZE_MIB:-512}"
IMAGE_SIZE_MIB="${LUNA_IMAGE_SIZE_MIB:-1280}"

mkdir -p "$DATA_ROOT/system/apps" "$DATA_ROOT/system/drivers" "$DATA_ROOT/system/libs" "$DATA_ROOT/system/volumes" "$DATA_ROOT/system/config" "$DATA_ROOT/system/state" "$DATA_ROOT/users/luna/home" "$DATA_ROOT/users/luna/data" "$DATA_ROOT/users/luna/config" "$DATA_ROOT/cache"
chown -R 1000:1000 "$DATA_ROOT/users/luna"
chmod 0700 "$DATA_ROOT/users/luna/home"
truncate -s "${DATA_SIZE_MIB}M" "$OUT/luna-data.img"
mkfs.ext4 -q -F -L LUNA-DATA -d "$DATA_ROOT" "$OUT/luna-data.img" >/dev/null

SYSTEM_SECTORS=$((SYSTEM_SIZE_MIB * 2048))
DATA_SECTORS=$((DATA_SIZE_MIB * 2048))
SYSTEM_START=264192
SYSTEM_END=$((SYSTEM_START + SYSTEM_SECTORS - 1))
DATA_START=$((SYSTEM_END + 1))
DATA_END=$((DATA_START + DATA_SECTORS - 1))
REQUIRED_SECTORS=$((DATA_END + 34))
REQUIRED_MIB=$(((REQUIRED_SECTORS + 2047) / 2048))
[ "$IMAGE_SIZE_MIB" -ge "$REQUIRED_MIB" ] || { echo "image size ${IMAGE_SIZE_MIB} MiB is too small; need at least ${REQUIRED_MIB} MiB" >&2; exit 1; }

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
truncate -s "${SYSTEM_SIZE_MIB}M" "$OUT/luna-system.img"
mkfs.ext4 -q -F -L LUNA-SYSTEM -d "$WORK/system-partition" "$OUT/luna-system.img" >/dev/null

truncate -s "${IMAGE_SIZE_MIB}M" "$OUT/luna-pc.img"
sgdisk --zap-all "$OUT/luna-pc.img" >/dev/null
sgdisk -n "1:2048:$((2048 + 128 * 2048 - 1))" -t 1:ef00 -c 1:EFI \
       -n "2:${SYSTEM_START}:${SYSTEM_END}" -t 2:8300 -c 2:SYSTEM \
       -n "3:${DATA_START}:${DATA_END}" -t 3:8300 -c 3:DATA "$OUT/luna-pc.img" >/dev/null
truncate -s 128M "$OUT/luna-efi.img"
mkfs.fat -F 32 "$OUT/luna-efi.img" >/dev/null
mmd -i "$OUT/luna-efi.img" ::/EFI; mmd -i "$OUT/luna-efi.img" ::/EFI/LUNA; mmd -i "$OUT/luna-efi.img" ::/EFI/BOOT
mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI; mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
dd if="$OUT/luna-efi.img" of="$OUT/luna-pc.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/luna-system.img" of="$OUT/luna-pc.img" bs=512 seek="$SYSTEM_START" conv=notrunc status=none
dd if="$OUT/luna-data.img" of="$OUT/luna-pc.img" bs=512 seek="$DATA_START" conv=notrunc status=none

cat > "$OUT/BUILD-INFO" <<EOF
Project Luna PC graphical development image
version=$LUNA_VERSION
architecture=x86_64
system_image=luna-${LUNA_VERSION}.squashfs
system_libc=musl
bootloader=luna-boot.efi
uefi_fallback=EFI/BOOT/BOOTX64.EFI
kernel_version=$KERNEL_VERSION
partitions=EFI:128MiB,SYSTEM:${SYSTEM_SIZE_MIB}MiB,DATA:${DATA_SIZE_MIB}MiB
image_size=${IMAGE_SIZE_MIB}MiB
boot_ui=graphical
login_ui=/usr/bin/luna-login
desktop=/usr/bin/niri-session
shell=/usr/bin/fish
terminal=/usr/bin/ghostty
compatibility_shells=/usr/bin/bash,/usr/bin/sh
verbose_boot=boot-menu-only
login_username=luna
login_credential=development-only
EOF
sha256sum "$OUT/luna-pc.img" "$OUT/luna-${LUNA_VERSION}.squashfs" "$OUT/luna-initramfs.img" > "$OUT/SHA256SUMS"

echo "Built Project Luna graphical PC image: $OUT/luna-pc.img"