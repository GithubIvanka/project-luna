#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
WORK="${OUT}/work"
SYSTEM_ROOT="${WORK}/system-root"
DATA_ROOT="${WORK}/data-root"
SYSTEM_PARTITION_ROOT="${WORK}/system-partition"

LUNA_VERSION="${LUNA_VERSION:-0.1.0}"
LUNA_INIT_VERSION="${LUNA_INIT_VERSION:-$LUNA_VERSION}"
LUNA_PASSWORD_HASH="${LUNA_PASSWORD_HASH:-}"
KERNEL="${LUNA_TEST_KERNEL:-}"
DESKTOP_ROOT="${LUNA_DESKTOP_ROOT:-}"

find_latest() {
    local pattern="$1"
    local value
    value=$(compgen -G "$pattern" | sort -V | tail -n 1 || true)
    [ -n "$value" ] && printf '%s\n' "$value"
}

if [ -z "$KERNEL" ]; then KERNEL="$(find_latest '/boot/vmlinuz-*' || true)"; fi

: "${KERNEL:?No Linux kernel found. Set LUNA_TEST_KERNEL to the built x86_64 Luna bzImage.}"
: "${DESKTOP_ROOT:?Luna PC images require a prepared graphical desktop payload staging root. Set LUNA_DESKTOP_ROOT to the prepared desktop payload root.}"

for tool in cargo rustup sgdisk mkfs.ext4 mkfs.fat mkswap mcopy mmd dd mksquashfs file truncate sha256sum; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

MKFS_BTRFS="${LUNA_MKFS_BTRFS:-$(command -v mkfs.btrfs || true)}"
: "${MKFS_BTRFS:?missing required tool: mkfs.btrfs (install btrfs-progs or set LUNA_MKFS_BTRFS)}"
[ -x "$MKFS_BTRFS" ] || { echo "LUNA_MKFS_BTRFS is not executable: $MKFS_BTRFS" >&2; exit 1; }
[ -f "$KERNEL" ] || { echo "kernel not found: $KERNEL" >&2; exit 1; }
[ -d "$DESKTOP_ROOT" ] || { echo "LUNA_DESKTOP_ROOT is not a directory: $DESKTOP_ROOT" >&2; exit 1; }
case "$OUT" in
    "$REPO_ROOT/dist"|"$REPO_ROOT/dist"/*) ;;
    *) echo "LUNA_OUT_DIR must stay inside $REPO_ROOT/dist" >&2; exit 1 ;;
esac
case "$KERNEL" in
    "$REPO_ROOT/dist"/*) ;;
    *) echo "LUNA_TEST_KERNEL must point to an artifact under $REPO_ROOT/dist" >&2; exit 1 ;;
esac
case "$DESKTOP_ROOT" in
    "$REPO_ROOT/dist"/*) ;;
    *) echo "LUNA_DESKTOP_ROOT must point to an artifact under $REPO_ROOT/dist" >&2; exit 1 ;;
esac
[ -x "$DESKTOP_ROOT/usr/bin/luna-user-session" ] || { echo "UserSession handoff binary missing: $DESKTOP_ROOT/usr/bin/luna-user-session" >&2; exit 1; }
[ -x "$DESKTOP_ROOT/usr/bin/niri-session" ] || { echo "niri session missing: $DESKTOP_ROOT/usr/bin/niri-session" >&2; exit 1; }

RUNTIME_TARGET="x86_64-unknown-linux-musl"
if ! rustup target list --installed | grep -qx "$RUNTIME_TARGET"; then rustup target add "$RUNTIME_TARGET"; fi
if ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1 && ! command -v musl-gcc >/dev/null 2>&1; then
    echo "A musl C linker is required (install musl-tools/musl-gcc)." >&2
    exit 1
fi

KERNEL_BASENAME="$(basename "$KERNEL")"
KERNEL_VERSION="${LUNA_KERNEL_VERSION:-}"
if [ -z "$KERNEL_VERSION" ] && [ -f "$(dirname "$KERNEL")/release" ]; then
    KERNEL_VERSION="$(<"$(dirname "$KERNEL")/release")"
fi
if [ -z "$KERNEL_VERSION" ]; then
    KERNEL_VERSION="${KERNEL_BASENAME#vmlinuz-}"
fi
[ -n "$KERNEL_VERSION" ] && [ "$KERNEL_VERSION" != "bzImage" ] || { echo "could not determine kernel version; set LUNA_KERNEL_VERSION or provide an artifact release file" >&2; exit 1; }

mkdir -p "$OUT" "$WORK"
rm -rf "$SYSTEM_ROOT" "$DATA_ROOT" "$SYSTEM_PARTITION_ROOT"
mkdir -p "$DATA_ROOT/system"/{apps,drivers,firmware,libs,config,state,volumes}
mkdir -p "$DATA_ROOT/users/luna"/{home,data,config}
mkdir -p "$DATA_ROOT/cache"
rm -f "$OUT"/luna-pc.img "$OUT"/luna-efi.img "$OUT"/luna-system.img "$OUT"/luna-data.img "$OUT"/luna-${LUNA_VERSION}.squashfs "$OUT"/BUILD-INFO "$OUT"/SHA256SUMS

cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
cargo build --release --manifest-path "$REPO_ROOT/components/system/luna-init/Cargo.toml" --target "$RUNTIME_TARGET"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/system/luna-init/target/$RUNTIME_TARGET/release/luna-init"
if [ ! -x "$LUNA_INIT" ]; then
    LUNA_INIT="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-init"
fi
[ -x "$RUNTIME" ] || { echo "runtime binary was not produced: $RUNTIME" >&2; exit 1; }
[ -x "$LUNA_INIT" ] || { echo "luna-init binary was not produced: $LUNA_INIT" >&2; exit 1; }
file "$RUNTIME" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-system-runtime is not statically linked; refusing to build PC image" >&2; exit 1; }
file "$LUNA_INIT" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-init is not statically linked; refusing to build PC image" >&2; exit 1; }
cargo build --release -p luna-user-session --bin luna-user-session --target "$RUNTIME_TARGET"
SESSION="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-user-session"
[ -x "$SESSION" ] || { echo "luna-user-session handoff binary was not produced: $SESSION" >&2; exit 1; }
file "$SESSION" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-user-session handoff binary is not statically linked; refusing to build PC image" >&2; exit 1; }

UNIX_CHKPWD_SOURCE="$(readlink -f "$DESKTOP_ROOT/sbin/unix_chkpwd" 2>/dev/null || true)"
if [ ! -x "$UNIX_CHKPWD_SOURCE" ]; then UNIX_CHKPWD_SOURCE="$(readlink -f /sbin/unix_chkpwd 2>/dev/null || true)"; fi
[ -x "$UNIX_CHKPWD_SOURCE" ] || { echo "unix_chkpwd helper is unavailable" >&2; exit 1; }
mkdir -p "$SYSTEM_ROOT/apps/unix_chkpwd"
cp -L "$UNIX_CHKPWD_SOURCE" "$SYSTEM_ROOT/apps/unix_chkpwd/unix_chkpwd"
chmod 2755 "$SYSTEM_ROOT/apps/unix_chkpwd/unix_chkpwd"

cargo build --release --target x86_64-unknown-uefi --manifest-path "$REPO_ROOT/boot/luna-boot/Cargo.toml"
EFI="$REPO_ROOT/boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi"
[ -f "$EFI" ] || { echo "UEFI loader was not produced: $EFI" >&2; exit 1; }

# Canonical System Image: only the six accepted top-level namespaces exist.
mkdir -p "$SYSTEM_ROOT"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$SYSTEM_ROOT/apps/luna-system-runtime" "$SYSTEM_ROOT/apps/luna-user-session" "$SYSTEM_ROOT/apps/unix_chkpwd"
mkdir -p "$SYSTEM_ROOT/config/luna" "$SYSTEM_ROOT/resources"/{fonts,icons,themes,cursors,locales,translations}

cp "$RUNTIME" "$SYSTEM_ROOT/apps/luna-system-runtime/luna-system-runtime"
chmod 0755 "$SYSTEM_ROOT/apps/luna-system-runtime/luna-system-runtime"
cp "$SESSION" "$SYSTEM_ROOT/apps/luna-user-session/luna-user-session"
chmod 0755 "$SYSTEM_ROOT/apps/luna-user-session/luna-user-session"

# The desktop builder produces a host-style staging root. It is only an
# input staging area; its Linux directory layout is never embedded in Luna.
cp -a "$DESKTOP_ROOT/usr/lib/." "$SYSTEM_ROOT/libs/"
cp -a "$DESKTOP_ROOT/usr/share/." "$SYSTEM_ROOT/resources/"
cp -a "$DESKTOP_ROOT/etc/." "$SYSTEM_ROOT/config/"

# The dynamic ELF loader is a runtime bootstrap resource, not a Linux root.
mkdir -p "$SYSTEM_ROOT/libs/loader"
LOADER_SOURCE="$(readlink -f "$DESKTOP_ROOT/lib64/ld-linux-x86-64.so.2")"
if [ ! -f "$LOADER_SOURCE" ]; then LOADER_SOURCE="/lib64/ld-linux-x86-64.so.2"; fi
[ -f "$LOADER_SOURCE" ] || { echo "ELF loader is unavailable" >&2; exit 1; }
rm -f "$SYSTEM_ROOT/libs/loader/ld-linux-x86-64.so.2"; cp -L "$LOADER_SOURCE" "$SYSTEM_ROOT/libs/loader/ld-linux-x86-64.so.2"

# Copy real ELF dependency files into the Luna libs namespace. This repairs
# staging-root symlinks whose targets were never copied by the host builder.
stage_elf_dependencies() {
    local source_root="$1"
    local queue="$WORK/elf-dependency-queue"
    : > "$queue"
    find "$source_root/usr/bin" "$source_root/usr/sbin" "$source_root/usr/libexec" \
        -type f -perm -0100 -print0 2>/dev/null >> "$queue"
    find "$source_root/usr/lib" -path '*/security/*.so' -type f -print0 2>/dev/null >> "$queue"
    printf '%s\0' "$UNIX_CHKPWD_SOURCE" >> "$queue"
    declare -A seen=()
    local processed=0
    while true; do
        local entries=()
        mapfile -d '' -t entries < "$queue" || true
        local total="${#entries[@]}"
        [ "$processed" -lt "$total" ] || break
        for ((index = processed; index < total; index++)); do
            local elf="${entries[index]}"
            local actual_elf
            actual_elf="$(readlink -f "$elf" 2>/dev/null || true)"
            [ -f "$actual_elf" ] || continue
            [[ ${seen[$actual_elf]+x} ]] && continue
            seen[$actual_elf]=1
            file "$actual_elf" | grep -q 'ELF' || continue
            while IFS= read -r dep; do
                [ -n "$dep" ] || continue
                [ -e "$dep" ] || continue
                case "$dep" in /usr/lib/*|/lib/*|/lib64/*) ;; *) continue ;; esac
                local actual rel dest
                actual="$(readlink -f "$dep" 2>/dev/null || true)"
                [ -f "$actual" ] || continue
                case "$dep" in
                    /lib64/*) rel="${dep#/lib64/}"; dest="$SYSTEM_ROOT/libs/loader/$rel" ;;
                    /usr/lib/*) rel="${dep#/usr/lib/}"; dest="$SYSTEM_ROOT/libs/$rel" ;;
                    /lib/*) rel="${dep#/lib/}"; dest="$SYSTEM_ROOT/libs/$rel" ;;
                esac
                mkdir -p "$(dirname "$dest")"
                rm -f "$dest"
                cp -L "$actual" "$dest"
                [[ ${seen[$actual]+x} ]] || printf '%s\0' "$actual" >> "$queue"
            done < <(ldd "$actual_elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib64|lib|usr\/lib)/ {print $1}')
        done
        processed=$total
    done
}
stage_elf_dependencies "$DESKTOP_ROOT"

# No unresolved host-Linux symlinks may survive inside an immutable image.
find "$SYSTEM_ROOT/libs" -xtype l -delete

# Remove build/development metadata from the runtime library namespace.
find "$SYSTEM_ROOT/libs" -type d -name pkgconfig -prune -exec rm -rf {} +
if [ -d "$SYSTEM_ROOT/libs/tmpfiles.d" ]; then
    mkdir -p "$SYSTEM_ROOT/config/tmpfiles.d"
    cp -a "$SYSTEM_ROOT/libs/tmpfiles.d/." "$SYSTEM_ROOT/config/tmpfiles.d/"
    rm -rf "$SYSTEM_ROOT/libs/tmpfiles.d"
fi

# Every executable is an independent immutable application resource.
# The source may be in usr/bin, usr/sbin or usr/libexec, but the physical
# System Image never reproduces those Linux root namespaces.
for source_dir in "$DESKTOP_ROOT/usr/bin" "$DESKTOP_ROOT/usr/sbin" "$DESKTOP_ROOT/usr/libexec"; do
    [ -d "$source_dir" ] || continue
    while IFS= read -r -d '' binary; do
        name="$(basename "$binary")"
        [ "$name" = "luna-user-session" ] && continue
        app="$SYSTEM_ROOT/apps/$name"
        mkdir -p "$app"
        cp -a "$binary" "$app/$name"
        if [ -f "$app/$name" ] && [ ! -L "$app/$name" ]; then
            chmod 0755 "$app/$name" 2>/dev/null || true
        fi
    done < <(find "$source_dir" -type f -perm -0100 -print0; find "$source_dir" -type l -print0)
done

# Runtime service definitions use the logical application view.
for service in network bluetooth removable-media; do
    [ -f "$SYSTEM_ROOT/config/luna/services/$service.toml" ] || continue
done
sed -i 's#/usr/sbin/NetworkManager#/usr/bin/NetworkManager#g' "$SYSTEM_ROOT/config/luna/services/network.toml" 2>/dev/null || true
sed -i 's#/usr/libexec/bluetooth/bluetoothd#/usr/bin/bluetoothd#g' "$SYSTEM_ROOT/config/luna/services/bluetooth.toml" 2>/dev/null || true
sed -i 's#/usr/libexec/udisks2/udisksd#/usr/bin/udisksd#g' "$SYSTEM_ROOT/config/luna/services/removable-media.toml" 2>/dev/null || true

# Immutable desktop/application configuration is owned by the config class;
# session entry records the canonical app resource rather than /usr paths.
cat > "$SYSTEM_ROOT/config/luna/desktop.toml" <<EOF
[desktop]
compositor = "niri"
shell = "noctalia"
terminal = "ghostty"
interactive_shell = "fish"
compatibility_shell = "bash"
posix_shell = "sh"

[session]
entry = "/usr/bin/luna-user-session --handoff"
EOF
printf "%s\n" "/usr/bin/luna-user-session --handoff" > "$SYSTEM_ROOT/config/luna/graphical-session"

# Authentication files are immutable bootstrap seeds only. luna-init copies them
# into mutable LUNA-DATA/system/state/auth on first boot. A password hash may be
# supplied for development images; otherwise the account is locked.
cat > "$SYSTEM_ROOT/config/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
greeter:x:995:995:Luna Greeter:/run/greetd:/bin/sh
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
EOF
SHADOW_GID="${LUNA_SHADOW_GID:-$(getent group shadow | awk -F: 'NR == 1 {print $3}')}"
[ -n "$SHADOW_GID" ] || { echo "shadow group GID is unavailable; set LUNA_SHADOW_GID" >&2; exit 1; }
printf '%s\n' \
    'root:x:0:' \
    "shadow:x:${SHADOW_GID}:" \
    'greeter:x:995:' \
    'luna:x:1000:' > "$SYSTEM_ROOT/config/group"
if [ -n "$LUNA_PASSWORD_HASH" ]; then
    printf 'root:!*:0:0:99999:7:::\ngreeter:!*:0:0:99999:7:::\nluna:%s:0:0:99999:7:::\n' "$LUNA_PASSWORD_HASH" > "$SYSTEM_ROOT/config/shadow"
else
    cat > "$SYSTEM_ROOT/config/shadow" <<'EOF'
root:!*:0:0:99999:7:::
greeter:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
EOF
fi
chmod 0644 "$SYSTEM_ROOT/config/passwd" "$SYSTEM_ROOT/config/group"
chmod 0600 "$SYSTEM_ROOT/config/shadow"
printf "%s\n" "passwd: files" "group: files" "shadow: files" "gshadow: files" "hosts: files dns" "services: files" "networks: files" "protocols: files" > "$SYSTEM_ROOT/config/nsswitch.conf"
chmod 0644 "$SYSTEM_ROOT/config/nsswitch.conf"

# glibc NSS modules are required by PAM for file-backed account lookup.
for NSS_LIB in /usr/lib/x86_64-linux-gnu/libnss_files.so.2 /usr/lib/x86_64-linux-gnu/libnss_dns.so.2 /usr/lib/x86_64-linux-gnu/libresolv.so.2; do
    [ -f "$NSS_LIB" ] || { echo "required NSS library missing: $NSS_LIB" >&2; exit 1; }
    NSS_REL="${NSS_LIB#/usr/lib/}"
    mkdir -p "$SYSTEM_ROOT/libs/$(dirname "$NSS_REL")"
    cp -L "$NSS_LIB" "$SYSTEM_ROOT/libs/$NSS_REL"
done

# DATA is mutable state and therefore never enters the System Image.


mksquashfs "$SYSTEM_ROOT" "$OUT/luna-${LUNA_VERSION}.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

SYSTEM_SIZE_MIB="${LUNA_SYSTEM_SIZE_MIB:-768}"
DATA_SIZE_MIB="${LUNA_DATA_SIZE_MIB:-512}"
SWAP_SIZE_MIB="${LUNA_SWAP_SIZE_MIB:-128}"
IMAGE_SIZE_MIB="${LUNA_IMAGE_SIZE_MIB:-1664}"
SYSTEM_SECTORS=$((SYSTEM_SIZE_MIB * 2048))
DATA_SECTORS=$((DATA_SIZE_MIB * 2048))
SWAP_SECTORS=$((SWAP_SIZE_MIB * 2048))
SYSTEM_START=264192
SYSTEM_END=$((SYSTEM_START + SYSTEM_SECTORS - 1))
DATA_START=$((SYSTEM_END + 1))
DATA_END=$((DATA_START + DATA_SECTORS - 1))
SWAP_START=$((DATA_END + 1))
SWAP_END=$((SWAP_START + SWAP_SECTORS - 1))
REQUIRED_SECTORS=$((SWAP_END + 34))
REQUIRED_MIB=$(((REQUIRED_SECTORS + 2047) / 2048))
[ "$IMAGE_SIZE_MIB" -ge "$REQUIRED_MIB" ] || { echo "image size ${IMAGE_SIZE_MIB} MiB is too small; need at least ${REQUIRED_MIB} MiB" >&2; exit 1; }

mkdir -p "$SYSTEM_PARTITION_ROOT/config" "$SYSTEM_PARTITION_ROOT/cores" "$SYSTEM_PARTITION_ROOT/images" "$SYSTEM_PARTITION_ROOT/kernels/$KERNEL_VERSION" "$SYSTEM_PARTITION_ROOT/recovery"
cp "$OUT/luna-${LUNA_VERSION}.squashfs" "$SYSTEM_PARTITION_ROOT/images/luna-${LUNA_VERSION}.squashfs"
cp "$LUNA_INIT" "$SYSTEM_PARTITION_ROOT/cores/luna-${LUNA_INIT_VERSION}.init"
chmod 0755 "$SYSTEM_PARTITION_ROOT/cores/luna-${LUNA_INIT_VERSION}.init"
cp "$KERNEL" "$SYSTEM_PARTITION_ROOT/kernels/$KERNEL_VERSION/bzImage"

INIT_MANIFEST_TMP="$WORK/luna-${LUNA_INIT_VERSION}.init.toml"
cat > "$INIT_MANIFEST_TMP" <<EOF
[init]
name = "luna-init"
version = "$LUNA_INIT_VERSION"

[architecture]
arch = "x86_64"

[kernels]
compatible = ["$KERNEL_VERSION"]
EOF
cp "$INIT_MANIFEST_TMP" "$SYSTEM_PARTITION_ROOT/cores/luna-${LUNA_INIT_VERSION}.toml"

MANIFEST_TMP="$WORK/luna-${LUNA_VERSION}.toml"
cat > "$MANIFEST_TMP" <<EOF
[image]
name = "luna"
version = "$LUNA_VERSION"
format = "squashfs"

[architecture]
arch = "x86_64"

[init]
compatible = ["$LUNA_INIT_VERSION"]
# bootstrap below
[bootstrap]
critical = ["/apps/luna-system-runtime/luna-system-runtime"]
EOF
cp "$MANIFEST_TMP" "$SYSTEM_PARTITION_ROOT/images/luna-${LUNA_VERSION}.toml"

cat > "$SYSTEM_PARTITION_ROOT/config/luna-data.toml" <<'EOF'
[data]
preferred_disk_guid = "__LUNA_DATA_DISK_GUID__"
preferred_partition_guid = "__LUNA_DATA_PARTITION_GUID__"
EOF
cat > "$SYSTEM_PARTITION_ROOT/config/boot-state.toml" <<EOF
[state]
format = 1
generation = 1

[targets.current]
image = "$LUNA_VERSION"
init = "$LUNA_INIT_VERSION"
kernel = "$KERNEL_VERSION"

[boot]
attempt_id = 0
previous_attempt_failed = false
fallback_depth = 0
failure_code = 0
EOF

DISK_GUID="$(cat /proc/sys/kernel/random/uuid)"
SYS_GUID="$(cat /proc/sys/kernel/random/uuid)"
DATA_GUID="$(cat /proc/sys/kernel/random/uuid)"
sed -i \
    -e "s/__LUNA_DATA_DISK_GUID__/$DISK_GUID/g" \
    -e "s/__LUNA_DATA_PARTITION_GUID__/$DATA_GUID/g" \
    "$SYSTEM_PARTITION_ROOT/config/luna-data.toml"

truncate -s "${DATA_SIZE_MIB}M" "$OUT/luna-data.img"
"$MKFS_BTRFS" -q -f -L LUNA-DATA --rootdir "$DATA_ROOT" "$OUT/luna-data.img" >/dev/null
truncate -s "${SYSTEM_SIZE_MIB}M" "$OUT/luna-system.img"
mkfs.ext4 -q -F -L LUNA-SYS -d "$SYSTEM_PARTITION_ROOT" "$OUT/luna-system.img" >/dev/null
rm -f "$OUT/luna-swap.img"
fallocate -l "${SWAP_SIZE_MIB}M" "$OUT/luna-swap.img"
chmod 0600 "$OUT/luna-swap.img"
mkswap -L SWAP "$OUT/luna-swap.img" >/dev/null

truncate -s "${IMAGE_SIZE_MIB}M" "$OUT/luna-pc.img"
sgdisk --zap-all "$OUT/luna-pc.img" >/dev/null
sgdisk --disk-guid="$DISK_GUID" \
       --partition-guid=2:"$SYS_GUID" \
       --partition-guid=3:"$DATA_GUID" \
       -n "1:2048:$((2048 + 128 * 2048 - 1))" -t 1:ef00 -c 1:EFI \
       -n "2:${SYSTEM_START}:${SYSTEM_END}" -t 2:8300 -c 2:LUNA-SYS \
       -n "3:${DATA_START}:${DATA_END}" -t 3:8300 -c 3:LUNA-DATA \
       -n "4:${SWAP_START}:${SWAP_END}" -t 4:8200 -c 4:SWAP "$OUT/luna-pc.img" >/dev/null
truncate -s 128M "$OUT/luna-efi.img"
mkfs.fat -F 32 "$OUT/luna-efi.img" >/dev/null
mmd -i "$OUT/luna-efi.img" ::/EFI; mmd -i "$OUT/luna-efi.img" ::/EFI/LUNA; mmd -i "$OUT/luna-efi.img" ::/EFI/BOOT
mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI; mcopy -i "$OUT/luna-efi.img" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
dd if="$OUT/luna-efi.img" of="$OUT/luna-pc.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$OUT/luna-system.img" of="$OUT/luna-pc.img" bs=512 seek="$SYSTEM_START" conv=notrunc status=none
dd if="$OUT/luna-data.img" of="$OUT/luna-pc.img" bs=512 seek="$DATA_START" conv=notrunc status=none
dd if="$OUT/luna-swap.img" of="$OUT/luna-pc.img" bs=512 seek="$SWAP_START" conv=notrunc status=none

cat > "$OUT/BUILD-INFO" <<EOF
Project Luna PC graphical development image
version=$LUNA_VERSION
architecture=x86_64
system_image=luna-${LUNA_VERSION}.squashfs
system_manifest=luna-${LUNA_VERSION}.toml
luna_init=luna-${LUNA_INIT_VERSION}.init
luna_init_manifest=luna-${LUNA_INIT_VERSION}.toml
system_libc=musl
bootloader=luna-boot.efi
uefi_fallback=EFI/BOOT/BOOTX64.EFI
kernel_version=$KERNEL_VERSION
partitions=EFI:128MiB,LUNA-SYS:${SYSTEM_SIZE_MIB}MiB,LUNA-DATA:${DATA_SIZE_MIB}MiB,SWAP:${SWAP_SIZE_MIB}MiB
image_size=${IMAGE_SIZE_MIB}MiB
boot_ui=graphical
user_session=/usr/bin/luna-user-session
desktop=/usr/bin/niri-session
shell=/usr/bin/fish
terminal=/usr/bin/ghostty
compatibility_shells=/usr/bin/bash,/usr/bin/sh
verbose_boot=boot-menu-only
login_username=luna
login_credential=development-only
early_userspace=direct-memory-resident-luna-init
EOF
sha256sum "$OUT/luna-pc.img" "$OUT/luna-${LUNA_VERSION}.squashfs" "$SYSTEM_PARTITION_ROOT/cores/luna-${LUNA_INIT_VERSION}.init" "$OUT/luna-system.img" "$OUT/luna-data.img" > "$OUT/SHA256SUMS"

echo "Built Project Luna graphical PC image: $OUT/luna-pc.img"
