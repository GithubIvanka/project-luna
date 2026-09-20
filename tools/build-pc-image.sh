#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
WORK="${OUT}/.build"
SYSTEM_ROOT="${WORK}/system-image-root"
LUNA_DATA_ROOT="${OUT}/luna-data"
LUNA_SYS_ROOT="${OUT}/luna-sys"
DATA_ROOT="$LUNA_DATA_ROOT"
SYSTEM_PARTITION_ROOT="$LUNA_SYS_ROOT"
PARTITION_WORK="${WORK}/partitions"

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

mkdir -p "$OUT" "$WORK" "$PARTITION_WORK"
rm -rf "$SYSTEM_ROOT" "$LUNA_DATA_ROOT" "$LUNA_SYS_ROOT"
mkdir -p "$DATA_ROOT/system"/{apps,drivers,firmware,libs,config,resources,state,volumes}
mkdir -p "$DATA_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
mkdir -p "$DATA_ROOT/users/luna"/{home,data,config}
mkdir -p "$DATA_ROOT/cache"
rm -f "$OUT"/luna-pc.img "$OUT"/luna-${LUNA_VERSION}.squashfs "$OUT"/BUILD-INFO "$OUT"/SHA256SUMS

cargo build --release -p luna-device-manager --target "$RUNTIME_TARGET"
cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
cargo build --release --manifest-path "$REPO_ROOT/components/core/luna-init/Cargo.toml" --target "$RUNTIME_TARGET"
DEVICE_MANAGER="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-device-manager"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/core/luna-init/target/$RUNTIME_TARGET/release/luna-init"
if [ ! -x "$LUNA_INIT" ]; then
    LUNA_INIT="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-init"
fi
[ -x "$DEVICE_MANAGER" ] || { echo "luna-device-manager binary was not produced: $DEVICE_MANAGER" >&2; exit 1; }
[ -x "$RUNTIME" ] || { echo "runtime binary was not produced: $RUNTIME" >&2; exit 1; }
[ -x "$LUNA_INIT" ] || { echo "luna-init binary was not produced: $LUNA_INIT" >&2; exit 1; }
file "$RUNTIME" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-system-runtime is not statically linked; refusing to build PC image" >&2; exit 1; }
file "$LUNA_INIT" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-init is not statically linked; refusing to build PC image" >&2; exit 1; }
cargo build --release -p luna-user-session --bin luna-user-session --target "$RUNTIME_TARGET"
SESSION="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-user-session"
[ -x "$SESSION" ] || { echo "luna-user-session handoff binary was not produced: $SESSION" >&2; exit 1; }
file "$SESSION" | grep -Eq 'statically linked|static-pie linked' || { echo "luna-user-session handoff binary is not statically linked; refusing to build PC image" >&2; exit 1; }

# Full desktop providers remain in DATA as complete functional packages.
# The boot-critical System Image contains only Luna-native core binaries.

cargo build --release --target x86_64-unknown-uefi --manifest-path "$REPO_ROOT/boot/luna-boot/Cargo.toml"
EFI="$REPO_ROOT/boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi"
[ -f "$EFI" ] || { echo "UEFI loader was not produced: $EFI" >&2; exit 1; }

# Canonical System Image: only the accepted top-level namespaces exist; resource types live under resources/.
# Recreate the build root so stale artifacts from an earlier image can never
# masquerade as current System Image content.
rm -rf "$SYSTEM_ROOT"
mkdir -p "$SYSTEM_ROOT"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$SYSTEM_ROOT/apps/luna-device-manager" "$SYSTEM_ROOT/apps/luna-system-runtime" "$SYSTEM_ROOT/apps/luna-user-session"
mkdir -p "$SYSTEM_ROOT/config/luna" "$SYSTEM_ROOT/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}

cp "$DEVICE_MANAGER" "$SYSTEM_ROOT/apps/luna-device-manager/luna-device-manager"
chmod 0755 "$SYSTEM_ROOT/apps/luna-device-manager/luna-device-manager"
cp "$RUNTIME" "$SYSTEM_ROOT/apps/luna-system-runtime/luna-system-runtime"
chmod 0755 "$SYSTEM_ROOT/apps/luna-system-runtime/luna-system-runtime"
cp "$SESSION" "$SYSTEM_ROOT/apps/luna-user-session/luna-user-session"
chmod 0755 "$SYSTEM_ROOT/apps/luna-user-session/luna-user-session"

# The desktop builder produces a host-style staging root. It is only an
# input staging area; its host directory layout is never embedded in Luna.
cp -a "$DESKTOP_ROOT/usr/lib/." "$DATA_ROOT/system/libs/"
cp -a "$DESKTOP_ROOT/etc/." "$DATA_ROOT/system/config/"

# System resources use the seven canonical resource types. Provider-specific
# runtime data that normally lives below /usr/share stays inside the owning
# application resource tree and is exposed by luna-init at its logical path.
copy_resource_dir() {
    local source="$1"
    local target="$2"
    [ -d "$source" ] || return 0
    mkdir -p "$target"
    cp -a "$source/." "$target/"
}
copy_resource_dir "$DESKTOP_ROOT/usr/share/fonts" "$DATA_ROOT/system/resources/fonts"
copy_resource_dir "$DESKTOP_ROOT/usr/share/icons" "$DATA_ROOT/system/resources/icons"
copy_resource_dir "$DESKTOP_ROOT/usr/share/themes" "$DATA_ROOT/system/resources/themes"
copy_resource_dir "$DESKTOP_ROOT/usr/share/locale" "$DATA_ROOT/system/resources/translations"
# Keep a separate canonical locale class even when the current desktop payload
# has no locale-definition files. Cursor payloads may be supplied directly here.
copy_resource_dir "$DESKTOP_ROOT/usr/share/cursors" "$DATA_ROOT/system/resources/cursors"
copy_resource_dir "$DESKTOP_ROOT/usr/share/noctalia/assets/sounds" "$DATA_ROOT/system/resources/sounds"
# The immutable System Image selects the Niri provider command.
# Recovery VirtualData uses the exact same command; only the DATA source changes.
for provider in noctalia wireplumber niri; do
    if [ -d "$DESKTOP_ROOT/usr/share/$provider" ]; then
        mkdir -p "$DATA_ROOT/system/apps/$provider/resources"
        cp -a "$DESKTOP_ROOT/usr/share/$provider" "$DATA_ROOT/system/apps/$provider/resources/$provider"
    fi
done
# GLVND uses data-driven vendor discovery outside the ELF dependency graph.
# Keep Mesa's vendor metadata as an application resource so luna-init exposes
# it at the canonical runtime path /usr/share/glvnd.
if [ -d "$DESKTOP_ROOT/usr/share/glvnd" ]; then
    mkdir -p "$DATA_ROOT/system/apps/niri/resources"
    cp -a "$DESKTOP_ROOT/usr/share/glvnd" "$DATA_ROOT/system/apps/niri/resources/glvnd"
fi
# Provider-specific desktop data must live under the application that owns it.
# luna-init exposes application resources at their logical runtime paths; the
# application directory remains the immutable ownership boundary.
if [ -d "$DESKTOP_ROOT/usr/share/libinput" ]; then
    mkdir -p "$DATA_ROOT/system/apps/niri/resources"
    cp -a "$DESKTOP_ROOT/usr/share/libinput" "$DATA_ROOT/system/apps/niri/resources/libinput"
fi
if [ -d "$DESKTOP_ROOT/usr/share/wayland-sessions" ]; then
    mkdir -p "$DATA_ROOT/system/apps/niri/resources"
    cp -a "$DESKTOP_ROOT/usr/share/wayland-sessions" "$DATA_ROOT/system/apps/niri/resources/wayland-sessions"
fi
if [ -d "$DESKTOP_ROOT/usr/share/applications" ]; then
    mkdir -p "$DATA_ROOT/system/apps/noctalia/resources"
    cp -a "$DESKTOP_ROOT/usr/share/applications" "$DATA_ROOT/system/apps/noctalia/resources/applications"
fi
if [ -d "$DESKTOP_ROOT/usr/share/polkit-1" ]; then
    mkdir -p "$DATA_ROOT/system/apps/dbus-daemon/resources"
    cp -a "$DESKTOP_ROOT/usr/share/polkit-1" "$DATA_ROOT/system/apps/dbus-daemon/resources/polkit-1"
fi
if [ -d "$DESKTOP_ROOT/usr/share/dbus-1" ]; then
    mkdir -p "$DATA_ROOT/system/apps/dbus-daemon/resources"
    cp -a "$DESKTOP_ROOT/usr/share/dbus-1" "$DATA_ROOT/system/apps/dbus-daemon/resources/dbus-1"
fi

# Every full desktop executable is a DATA application. This keeps Niri,
# Noctalia, Ghostty, Yazi and their helpers intact outside the boot-critical image.
for source_dir in "$DESKTOP_ROOT/usr/bin" "$DESKTOP_ROOT/usr/sbin" "$DESKTOP_ROOT/usr/libexec"; do
    [ -d "$source_dir" ] || continue
    while IFS= read -r -d '' binary; do
        name="$(basename "$binary")"
        case "$name" in
            luna-device-manager|luna-system-runtime|luna-user-session|greetd*|noctalia-greeter*|unix_chkpwd|dbus-run-session|seatd|niri-session)
                continue
                ;;
        esac
        app="$DATA_ROOT/system/apps/$name"
        mkdir -p "$app"
        cp -L "$binary" "$app/$name"
        chmod 0755 "$app/$name" 2>/dev/null || true
    done < <(find "$source_dir" -type f -perm -0100 -print0; find "$source_dir" -type l -print0)
done

# The Luna graphical session is created natively by luna-system-runtime.
# Keep the retired greetd/Noctalia-greeter login path out of DATA even if an
# upstream payload unexpectedly reintroduces one of its files.
legacy_desktop_files="$(find "$DATA_ROOT/system" \( -type f -o -type l -o -type d \) -print | grep -E '/(greetd|noctalia-greeter)(/|$)' || true)"
if [ -n "$legacy_desktop_files" ]; then
    echo "legacy greetd/Noctalia-greeter payload detected in LUNA-DATA:" >&2
    printf '%s\n' "$legacy_desktop_files" >&2
    exit 1
fi

# The dynamic ELF loader belongs to the DATA provider runtime closure.
mkdir -p "$DATA_ROOT/system/libs/loader"
LOADER_SOURCE="$(readlink -f "$DESKTOP_ROOT/lib64/ld-linux-x86-64.so.2")"
if [ ! -f "$LOADER_SOURCE" ]; then LOADER_SOURCE="/lib64/ld-linux-x86-64.so.2"; fi
[ -f "$LOADER_SOURCE" ] || { echo "ELF loader is unavailable" >&2; exit 1; }
rm -f "$DATA_ROOT/system/libs/loader/ld-linux-x86-64.so.2"
cp -L "$LOADER_SOURCE" "$DATA_ROOT/system/libs/loader/ld-linux-x86-64.so.2"

# Copy real ELF dependency files into the Luna libs namespace. This repairs
# staging-root symlinks whose targets were never copied by the host builder.
stage_elf_dependencies() {
    local source_root="$1"
    local queue="$WORK/elf-dependency-queue"
    : > "$queue"
    for search_root in "$source_root/usr/bin" "$source_root/usr/sbin" "$source_root/usr/libexec"; do
        [ -d "$search_root" ] || continue
        find "$search_root" -type f -perm -0100 -print0 2>/dev/null >> "$queue"
        find "$search_root" -type l -print0 2>/dev/null >> "$queue"
    done
    if [ -d "$source_root/usr/lib" ]; then
        find "$source_root/usr/lib" -path '*/security/*.so' -type f -print0 2>/dev/null >> "$queue"
    fi
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
                    /lib64/*) rel="${dep#/lib64/}"; dest="$DATA_ROOT/system/libs/loader/$rel" ;;
                    /usr/lib/*) rel="${dep#/usr/lib/}"; dest="$DATA_ROOT/system/libs/$rel" ;;
                    /lib/*) rel="${dep#/lib/}"; dest="$DATA_ROOT/system/libs/$rel" ;;
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
find "$DATA_ROOT/system/libs" -xtype l -delete

# Remove build/development metadata from the runtime library namespace.
find "$DATA_ROOT/system/libs" -type d -name pkgconfig -prune -exec rm -rf {} +
if [ -d "$DATA_ROOT/system/libs/tmpfiles.d" ]; then
    mkdir -p "$DATA_ROOT/system/config/tmpfiles.d"
    cp -a "$DATA_ROOT/system/libs/tmpfiles.d/." "$DATA_ROOT/system/config/tmpfiles.d/"
    rm -rf "$DATA_ROOT/system/libs/tmpfiles.d"
fi

# Desktop executables are already copied into DATA/system/apps above.
# Runtime service definitions are DATA-owned configuration.

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
# System Image contains the immutable default provider selection.
# Physical/Virtual DATA may override the same path for a selected environment.

# Authentication files are immutable bootstrap seeds only. luna-init copies them
# into mutable LUNA-DATA/system/state/auth on first boot. A password hash may be
# supplied for development images; otherwise the account is locked.
cat > "$SYSTEM_ROOT/config/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
messagebus:x:995:995:System Message Bus:/nonexistent:/usr/sbin/nologin
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
EOF
SHADOW_GID="${LUNA_SHADOW_GID:-$(getent group shadow | awk -F: 'NR == 1 {print $3}')}"
[ -n "$SHADOW_GID" ] || { echo "shadow group GID is unavailable; set LUNA_SHADOW_GID" >&2; exit 1; }
MESSAGEBUS_UID=995
MESSAGEBUS_GID=995
printf '%s\n' \
    'root:x:0:' \
    "shadow:x:${SHADOW_GID}:" \
    "messagebus:x:${MESSAGEBUS_GID}:" \
    'seat:x:997:luna' \
    'input:x:998:luna' \
    'luna:x:1000:' > "$SYSTEM_ROOT/config/group"
if [ -n "$LUNA_PASSWORD_HASH" ]; then
    printf 'root:!*:0:0:99999:7:::\nluna:%s:0:0:99999:7:::\n' "$LUNA_PASSWORD_HASH" > "$SYSTEM_ROOT/config/shadow"
else
    cat > "$SYSTEM_ROOT/config/shadow" <<'EOF'
root:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
EOF
fi
chmod 0644 "$SYSTEM_ROOT/config/passwd" "$SYSTEM_ROOT/config/group"
chmod 0600 "$SYSTEM_ROOT/config/shadow"
printf "%s\n" "passwd: files" "group: files" "shadow: files" "gshadow: files" "hosts: files dns" "services: files" "networks: files" "protocols: files" > "$SYSTEM_ROOT/config/nsswitch.conf"
chmod 0644 "$SYSTEM_ROOT/config/nsswitch.conf"

# Guard the architectural boundary: desktop/provider executables and their
# dynamic runtime closure must never leak into the boot-critical System Image.
if find "$SYSTEM_ROOT/apps" -mindepth 1 -maxdepth 1 -type d \
    ! -name luna-device-manager ! -name luna-system-runtime ! -name luna-user-session \
    -print -quit | grep -q .; then
    echo "unexpected non-core application in System Image" >&2
    exit 1
fi
if find "$SYSTEM_ROOT/libs" -type f -o -type l | grep -q .; then
    echo "unexpected library payload in System Image; compositor/provider closure belongs in DATA or Recovery DATA" >&2
    exit 1
fi
# D-Bus and several host services expect a stable machine identity. Derive a
# per-installation identity during boot from the persistent SYSTEM partition
# GUID rather than baking the builder host's /etc/machine-id into the image.

# glibc NSS modules are compatibility dependencies for the transitional
# DATA authentication/provider stack; they do not belong in System Image.
for NSS_LIB in /usr/lib/x86_64-linux-gnu/libnss_files.so.2 /usr/lib/x86_64-linux-gnu/libnss_dns.so.2 /usr/lib/x86_64-linux-gnu/libresolv.so.2; do
    [ -f "$NSS_LIB" ] || { echo "required NSS library missing: $NSS_LIB" >&2; exit 1; }
    NSS_REL="${NSS_LIB#/usr/lib/}"
    mkdir -p "$DATA_ROOT/system/libs/$(dirname "$NSS_REL")"
    cp -L "$NSS_LIB" "$DATA_ROOT/system/libs/$NSS_REL"
done

# DATA is mutable state and therefore never enters the System Image.


# Recovery DATA is the only recovery-specific filesystem. It is intentionally
# small and self-contained; luna-init materializes it into a writable tmpfs
# VirtualData provider when boot mode is Recovery.
RECOVERY_DATA_ROOT="$WORK/recovery-data-root"
RECOVERY_DATA_IMAGE="$WORK/recovery.squashfs"
rm -rf "$RECOVERY_DATA_ROOT"
mkdir -p "$RECOVERY_DATA_ROOT/system"/{apps,drivers,firmware,libs,config,state,volumes}
mkdir -p "$RECOVERY_DATA_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
mkdir -p "$RECOVERY_DATA_ROOT/system/state/auth"
mkdir -p "$RECOVERY_DATA_ROOT/system/apps"/{recovery-tools,data-discovery,system-diagnostics,system-repair}
mkdir -p "$RECOVERY_DATA_ROOT/users/recovery"/{home,data,config}
mkdir -p "$RECOVERY_DATA_ROOT/cache"

# Recovery uses the same graphical provider family as normal boot. The
# compositor is Niri in both cases; only the DATA provider changes from the
# persistent LUNA-DATA filesystem to the in-RAM Recovery DATA image.
mkdir -p "$RECOVERY_DATA_ROOT/system/apps"/{niri,noctalia,ghostty,dbus-daemon,pipewire,wireplumber}
mkdir -p "$RECOVERY_DATA_ROOT/system/libs/x86_64-linux-gnu" "$RECOVERY_DATA_ROOT/system/libs/loader"

for app in niri noctalia ghostty dbus-daemon pipewire wireplumber; do
    source="$DATA_ROOT/system/apps/$app/$app"
    if [ -f "$source" ] || [ -L "$source" ]; then
        cp -L "$source" "$RECOVERY_DATA_ROOT/system/apps/$app/$app"
        chmod 0755 "$RECOVERY_DATA_ROOT/system/apps/$app/$app" 2>/dev/null || true
    fi
done
for app in niri noctalia ghostty; do
    if [ -d "$DATA_ROOT/system/apps/$app/resources" ]; then
        cp -a "$DATA_ROOT/system/apps/$app/resources" "$RECOVERY_DATA_ROOT/system/apps/$app/"
    fi
done

# Niri reads its configuration from the logical /etc/luna path materialized
# from Recovery DATA/system/config by luna-init, just like normal DATA.
if [ -f "$DATA_ROOT/system/config/luna/niri-config.kdl" ]; then
    mkdir -p "$RECOVERY_DATA_ROOT/system/config/luna"
    cp -a "$DATA_ROOT/system/config/luna/niri-config.kdl" \
        "$RECOVERY_DATA_ROOT/system/config/luna/niri-config.kdl"
fi

RECOVERY_QUEUE="$WORK/recovery-niri-elf-queue"
: > "$RECOVERY_QUEUE"
for app in niri noctalia ghostty dbus-daemon pipewire wireplumber; do
    source="$RECOVERY_DATA_ROOT/system/apps/$app/$app"
    [ -f "$source" ] || continue
    printf '%s\0' "$source" >> "$RECOVERY_QUEUE"
done
RECOVERY_LOADER="$(readlink -f "$DATA_ROOT/system/libs/loader/ld-linux-x86-64.so.2" 2>/dev/null || true)"
[ -f "$RECOVERY_LOADER" ] || { echo "glibc ELF loader is unavailable for Recovery Niri" >&2; exit 1; }
cp -L "$RECOVERY_LOADER" "$RECOVERY_DATA_ROOT/system/libs/loader/ld-linux-x86-64.so.2"

declare -A RECOVERY_SEEN=()
RECOVERY_PROCESSED=0
while true; do
    mapfile -d '' -t RECOVERY_ENTRIES < "$RECOVERY_QUEUE" || true
    RECOVERY_TOTAL="${#RECOVERY_ENTRIES[@]}"
    [ "$RECOVERY_PROCESSED" -lt "$RECOVERY_TOTAL" ] || break
    for ((index = RECOVERY_PROCESSED; index < RECOVERY_TOTAL; index++)); do
        RECOVERY_ELF="${RECOVERY_ENTRIES[index]}"
        RECOVERY_ACTUAL="$(readlink -f "$RECOVERY_ELF" 2>/dev/null || true)"
        [ -f "$RECOVERY_ACTUAL" ] || continue
        [[ ${RECOVERY_SEEN[$RECOVERY_ACTUAL]+x} ]] && continue
        RECOVERY_SEEN[$RECOVERY_ACTUAL]=1
        file "$RECOVERY_ACTUAL" | grep -q 'ELF' || continue
        while IFS= read -r RECOVERY_DEP; do
            [ -n "$RECOVERY_DEP" ] || continue
            [ -f "$RECOVERY_DEP" ] || continue
            RECOVERY_DEP_BASE="$(basename "$RECOVERY_DEP")"
            RECOVERY_ACTUAL_DEP="$(readlink -f "$RECOVERY_DEP" 2>/dev/null || true)"
            [ -f "$RECOVERY_ACTUAL_DEP" ] || continue
            case "$RECOVERY_DEP_BASE" in
                ld-linux-*.so.*) RECOVERY_DEST="$RECOVERY_DATA_ROOT/system/libs/loader/$RECOVERY_DEP_BASE" ;;
                *) RECOVERY_DEST="$RECOVERY_DATA_ROOT/system/libs/x86_64-linux-gnu/$RECOVERY_DEP_BASE" ;;
            esac
            mkdir -p "$(dirname "$RECOVERY_DEST")"
            cp -L "$RECOVERY_ACTUAL_DEP" "$RECOVERY_DEST"
            [[ ${RECOVERY_SEEN[$RECOVERY_ACTUAL_DEP]+x} ]] || printf '%s\0' "$RECOVERY_ACTUAL_DEP" >> "$RECOVERY_QUEUE"
        done < <(
            LD_LIBRARY_PATH="$DATA_ROOT/system/libs/x86_64-linux-gnu:$REPO_ROOT/dist/.build/desktop-payload/usr/lib/x86_64-linux-gnu" \
                ldd "$RECOVERY_ACTUAL" 2>/dev/null |
                awk '/=> \// {print $3} /^\// {print $1}'
        )
    done
    RECOVERY_PROCESSED="$RECOVERY_TOTAL"
done

if [ -d "$DATA_ROOT/system/apps/niri/resources/wayland-sessions" ]; then
    mkdir -p "$RECOVERY_DATA_ROOT/system/apps/niri/resources"
    cp -a "$DATA_ROOT/system/apps/niri/resources/wayland-sessions" "$RECOVERY_DATA_ROOT/system/apps/niri/resources/"
fi
if [ -d "$DATA_ROOT/system/apps/niri/resources/X11" ]; then
    mkdir -p "$RECOVERY_DATA_ROOT/system/apps/niri/resources"
    cp -a "$DATA_ROOT/system/apps/niri/resources/X11" "$RECOVERY_DATA_ROOT/system/apps/niri/resources/"
fi
mkdir -p "$RECOVERY_DATA_ROOT/system/config/luna"
cat > "$RECOVERY_DATA_ROOT/system/config/luna/desktop.toml" <<'EOF'
[desktop]
compositor = "niri"
shell = "noctalia"
terminal = "ghostty"
EOF

cat > "$RECOVERY_DATA_ROOT/system/apps/recovery-tools/recovery-tools" <<'EOF'
#!/usr/bin/sh
printf '%s\n' 'Project Luna Recovery Tools'
printf '%s\n' 'Use data-discovery, system-diagnostics or system-repair.'
EOF
cat > "$RECOVERY_DATA_ROOT/system/apps/data-discovery/data-discovery" <<'EOF'
#!/usr/bin/sh
printf '%s\n' 'LUNA-DATA candidates:'
for path in /dev/disk/by-partuuid/*; do
    [ -e "$path" ] || continue
    printf '%s\n' "$path"
done
EOF
cat > "$RECOVERY_DATA_ROOT/system/apps/system-diagnostics/system-diagnostics" <<'EOF'
#!/usr/bin/sh
printf '%s\n' 'Project Luna system diagnostics'
printf 'kernel: '; uname -r 2>/dev/null || printf '%s\n' unavailable
printf 'recovery-data: mounted as VirtualData\n'
EOF
cat > "$RECOVERY_DATA_ROOT/system/apps/system-repair/system-repair" <<'EOF'
#!/usr/bin/sh
printf '%s\n' 'Project Luna system repair tools are available through the Recovery environment.'
printf '%s\n' 'Physical LUNA-DATA remains a separate repair target.'
EOF
chmod 0755 "$RECOVERY_DATA_ROOT"/system/apps/*/*

cat > "$RECOVERY_DATA_ROOT/system/state/recovery.toml" <<EOF
[recovery]
version = "$LUNA_VERSION"
provider = "VirtualData"
EOF
cat > "$RECOVERY_DATA_ROOT/system/state/auth/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
recovery:x:1001:1001:Luna Recovery:/home/recovery:/bin/sh
EOF
cat > "$RECOVERY_DATA_ROOT/system/state/auth/group" <<'EOF'
root:x:0:
shadow:x:42:
seat:x:997:recovery
recovery:x:1001:
EOF
if [ -n "$LUNA_PASSWORD_HASH" ]; then
    printf '%s\n' \
        'root:!*:0:0:99999:7:::' \
        "recovery:${LUNA_PASSWORD_HASH}:0:0:99999:7:::" > "$RECOVERY_DATA_ROOT/system/state/auth/shadow"
else
    cat > "$RECOVERY_DATA_ROOT/system/state/auth/shadow" <<'EOF'
root:!*:0:0:99999:7:::
recovery:!*:0:0:99999:7:::
EOF
fi
chmod 0644 "$RECOVERY_DATA_ROOT/system/state/auth/passwd" "$RECOVERY_DATA_ROOT/system/state/auth/group"
chmod 0600 "$RECOVERY_DATA_ROOT/system/state/auth/shadow"
for dir in "$RECOVERY_DATA_ROOT"/system/resources/*; do :; done

mksquashfs "$RECOVERY_DATA_ROOT" "$RECOVERY_DATA_IMAGE" -noappend -comp zstd -all-root -no-xattrs >/dev/null
mksquashfs "$SYSTEM_ROOT" "$OUT/luna-${LUNA_VERSION}.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

SYSTEM_SIZE_MIB="${LUNA_SYSTEM_SIZE_MIB:-768}"
DATA_SIZE_MIB="${LUNA_DATA_SIZE_MIB:-1280}"
SWAP_SIZE_MIB="${LUNA_SWAP_SIZE_MIB:-128}"
IMAGE_SIZE_MIB="${LUNA_IMAGE_SIZE_MIB:-2432}"

# LUNA-DATA is a real btrfs filesystem image. Its backing partition must be
# at least as large as the filesystem we create; otherwise desktop automount
# sees the correct GPT partition but btrfs reports a truncated device. Keep a
# small amount of free space in the default image for mutable runtime state.
DATA_ROOT_BYTES="$(du -sb "$DATA_ROOT" | awk '{print $1}')"
DATA_MIN_MIB="$(((DATA_ROOT_BYTES + 128 * 1024 * 1024 + 1024 * 1024 - 1) / (1024 * 1024)))"
[ "$DATA_SIZE_MIB" -ge "$DATA_MIN_MIB" ] || DATA_SIZE_MIB="$DATA_MIN_MIB"
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
cp "$RECOVERY_DATA_IMAGE" "$SYSTEM_PARTITION_ROOT/recovery/recovery.squashfs"
cat > "$SYSTEM_PARTITION_ROOT/recovery/recovery.toml" <<EOF
[image]
name = "recovery-data"
version = "$LUNA_VERSION"
format = "squashfs"
role = "recovery"

[architecture]
arch = "x86_64"

[init]
compatible = ["$LUNA_INIT_VERSION"]
EOF
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

[targets.recovery]
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

truncate -s "${DATA_SIZE_MIB}M" "$PARTITION_WORK/luna-data.fs"
"$MKFS_BTRFS" -q -f -L LUNA-DATA --rootdir "$DATA_ROOT" "$PARTITION_WORK/luna-data.fs" >/dev/null
DATA_FS_BYTES="$(stat -c '%s' "$PARTITION_WORK/luna-data.fs")"
DATA_PARTITION_BYTES=$((DATA_SECTORS * 512))
[ "$DATA_FS_BYTES" -le "$DATA_PARTITION_BYTES" ] || {
    echo "LUNA-DATA filesystem ($DATA_FS_BYTES bytes) exceeds partition ($DATA_PARTITION_BYTES bytes)" >&2
    exit 1
}
truncate -s "${SYSTEM_SIZE_MIB}M" "$PARTITION_WORK/luna-sys.fs"
mkfs.ext4 -q -F -L LUNA-SYS -d "$SYSTEM_PARTITION_ROOT" "$PARTITION_WORK/luna-sys.fs" >/dev/null
rm -f "$PARTITION_WORK/swap.fs"
fallocate -l "${SWAP_SIZE_MIB}M" "$PARTITION_WORK/swap.fs"
chmod 0600 "$PARTITION_WORK/swap.fs"
mkswap -L SWAP "$PARTITION_WORK/swap.fs" >/dev/null

truncate -s "${IMAGE_SIZE_MIB}M" "$OUT/luna-pc.img"
sgdisk --zap-all "$OUT/luna-pc.img" >/dev/null
sgdisk --disk-guid="$DISK_GUID" \
       -n "1:2048:$((2048 + 128 * 2048 - 1))" -t 1:ef00 -c 1:EFI \
       -n "2:${SYSTEM_START}:${SYSTEM_END}" -t 2:8300 -c 2:LUNA-SYS \
       -n "3:${DATA_START}:${DATA_END}" -t 3:8300 -c 3:LUNA-DATA \
       -n "4:${SWAP_START}:${SWAP_END}" -t 4:8200 -c 4:SWAP "$OUT/luna-pc.img" >/dev/null
# Partition GUIDs must be applied after the partitions exist. In particular,
# LUNA-DATA's GUID is part of the fast-attach manifest contract.
sgdisk --partition-guid=2:"$SYS_GUID" --partition-guid=3:"$DATA_GUID" "$OUT/luna-pc.img" >/dev/null
truncate -s 128M "$PARTITION_WORK/efi.fs"
mkfs.fat -F 32 "$PARTITION_WORK/efi.fs" >/dev/null
mmd -i "$PARTITION_WORK/efi.fs" ::/EFI; mmd -i "$PARTITION_WORK/efi.fs" ::/EFI/LUNA; mmd -i "$PARTITION_WORK/efi.fs" ::/EFI/BOOT
mcopy -i "$PARTITION_WORK/efi.fs" "$EFI" ::/EFI/LUNA/LUNA-BOOT.EFI; mcopy -i "$PARTITION_WORK/efi.fs" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
dd if="$PARTITION_WORK/efi.fs" of="$OUT/luna-pc.img" bs=512 seek=2048 conv=notrunc status=none
dd if="$PARTITION_WORK/luna-sys.fs" of="$OUT/luna-pc.img" bs=512 seek="$SYSTEM_START" conv=notrunc status=none
dd if="$PARTITION_WORK/luna-data.fs" of="$OUT/luna-pc.img" bs=512 seek="$DATA_START" conv=notrunc status=none
dd if="$PARTITION_WORK/swap.fs" of="$OUT/luna-pc.img" bs=512 seek="$SWAP_START" conv=notrunc status=none

# Verify the physical GPT identity against the manifest written into LUNA-SYS.
ACTUAL_DISK_GUID="$(sgdisk -p "$OUT/luna-pc.img" | awk -F': ' '/Disk identifier \(GUID\):/ {print tolower($2); exit}')"
ACTUAL_DATA_GUID="$(sgdisk -i 3 "$OUT/luna-pc.img" | awk -F': ' '/Partition unique GUID:/ {print tolower($2); exit}')"
MANIFEST_DISK_GUID="$(awk -F' = ' '/^preferred_disk_guid/ {gsub(/"/, "", $2); print tolower($2)}' "$SYSTEM_PARTITION_ROOT/config/luna-data.toml")"
MANIFEST_DATA_GUID="$(awk -F' = ' '/^preferred_partition_guid/ {gsub(/"/, "", $2); print tolower($2)}' "$SYSTEM_PARTITION_ROOT/config/luna-data.toml")"
[ "$ACTUAL_DISK_GUID" = "$MANIFEST_DISK_GUID" ] || { echo "LUNA-DATA manifest disk GUID mismatch: manifest=$MANIFEST_DISK_GUID GPT=$ACTUAL_DISK_GUID" >&2; exit 1; }
[ "$ACTUAL_DATA_GUID" = "$MANIFEST_DATA_GUID" ] || { echo "LUNA-DATA manifest partition GUID mismatch: manifest=$MANIFEST_DATA_GUID GPT=$ACTUAL_DATA_GUID" >&2; exit 1; }

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
desktop_provider_selection=Niri-direct-native-session
recovery_provider_selection=Niri-direct-native-session-VirtualData
shell=/usr/bin/fish
terminal=/usr/bin/ghostty
compatibility_shells=/usr/bin/bash,/usr/bin/sh
verbose_boot=boot-menu-only
login_username=luna
login_credential=development-only
early_userspace=direct-memory-resident-luna-init
EOF
sha256sum "$OUT/luna-pc.img" "$OUT/luna-${LUNA_VERSION}.squashfs" "$SYSTEM_PARTITION_ROOT/cores/luna-${LUNA_INIT_VERSION}.init" "$PARTITION_WORK/luna-sys.fs" "$PARTITION_WORK/luna-data.fs" > "$OUT/SHA256SUMS"

echo "Built Project Luna graphical PC image: $OUT/luna-pc.img"
