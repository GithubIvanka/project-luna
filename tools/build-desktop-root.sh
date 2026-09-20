#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_DESKTOP_ROOT_OUT:-${REPO_ROOT}/dist/.build/desktop-payload}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"
MESON_BUILD_SUFFIX="$(basename "$OUT")"
NIRI_TARGET_DIR="$SRC/niri/target-${MESON_BUILD_SUFFIX}"

NIRI_TAG="${LUNA_NIRI_TAG:-v26.04}"
NOCTALIA_TAG="${LUNA_NOCTALIA_TAG:-v5.0.0-beta.8}"
GHOSTTY_TAG="${LUNA_GHOSTTY_TAG:-v1.3.1}"
GHOSTTY_ZIG_VERSION="${LUNA_GHOSTTY_ZIG_VERSION:-0.15.2}"
FISH_VERSION="${LUNA_FISH_VERSION:-4.8.1}"
WAYLAND_VERSION="${LUNA_WAYLAND_VERSION:-1.26.0}"
WAYLAND_PROTOCOLS_VERSION="${LUNA_WAYLAND_PROTOCOLS_VERSION:-1.49}"
WLROOTS_VERSION="${LUNA_WLROOTS_VERSION:-0.20.2}"
WIREPLUMBER_VERSION="${LUNA_WIREPLUMBER_VERSION:-0.5.13}"

mkdir -p "$OUT" "$SRC"
OUT="$(cd "$OUT" && pwd)"
SRC="$(cd "$SRC" && pwd)"
MESON_BUILD_SUFFIX="$(basename "$OUT")"
NIRI_TARGET_DIR="$SRC/niri/target-${MESON_BUILD_SUFFIX}"
for _root_var in LUNA_NIRI_DEV_ROOT LUNA_PIPEWIRE_DEV_ROOT LUNA_SDBUS_DEV_ROOT LUNA_LIBRSVG_DEV_ROOT LUNA_NOCTALIA_DEV_ROOT LUNA_GMP_DEV_ROOT LUNA_MPFR_DEV_ROOT LUNA_GTK4_LAYER_SHELL_DEV_ROOT; do
    if [ -v "$_root_var" ]; then
        declare -n _root_ref="$_root_var"
        _root_ref="$(cd "$_root_ref" && pwd)"
    fi
done
unset _root_var _root_ref
rm -rf "$OUT"
mkdir -p "$OUT/usr/bin" "$OUT/usr/lib" "$OUT/usr/share" "$OUT/etc/profile.d" "$OUT/etc/luna"

fetch_git() {
    local url="$1" ref="$2" dir="$3"
    if [ ! -d "$dir/.git" ]; then
        git clone --filter=blob:none --no-tags "$url" "$dir"
    fi
    if [ "${LUNA_OFFLINE_SOURCES:-0}" = "1" ]; then
        echo "using existing source checkout for $url at $(git -C "$dir" rev-parse HEAD)" >&2
        return 0
    fi
    if git -C "$dir" fetch --depth 1 origin "$ref"; then
        git -C "$dir" checkout --force FETCH_HEAD
    else
        echo "warning: unable to refresh $url at $ref; using existing checkout $(git -C "$dir" rev-parse HEAD)" >&2
    fi
}

fetch_git https://github.com/niri-wm/niri.git "$NIRI_TAG" "$SRC/niri"
fetch_git https://github.com/noctalia-dev/noctalia.git "$NOCTALIA_TAG" "$SRC/noctalia"
fetch_git https://github.com/ghostty-org/ghostty.git "$GHOSTTY_TAG" "$SRC/ghostty"
fetch_git https://gitlab.freedesktop.org/wayland/wayland.git "$WAYLAND_VERSION" "$SRC/wayland"
fetch_git https://gitlab.freedesktop.org/wayland/wayland-protocols.git "$WAYLAND_PROTOCOLS_VERSION" "$SRC/wayland-protocols"
fetch_git https://gitlab.freedesktop.org/wlroots/wlroots.git "$WLROOTS_VERSION" "$SRC/wlroots"
fetch_git https://gitlab.freedesktop.org/pipewire/wireplumber.git "$WIREPLUMBER_VERSION" "$SRC/wireplumber"

FISH_DIR="$SRC/fish-$FISH_VERSION-linux-x86_64"
FISH_ARCHIVE="$SRC/fish-$FISH_VERSION-linux-x86_64.tar.xz"
if [ ! -x "$FISH_DIR/usr/bin/fish" ]; then
    rm -rf "$FISH_DIR"
    mkdir -p "$FISH_DIR/usr/bin"
    if [ ! -f "$FISH_ARCHIVE" ]; then
        curl -fsSL "https://github.com/fish-shell/fish-shell/releases/download/$FISH_VERSION/fish-$FISH_VERSION-linux-x86_64.tar.xz" -o "$FISH_ARCHIVE"
    fi
    if tar -tf "$FISH_ARCHIVE" | grep -qx 'fish'; then
        tar -xOf "$FISH_ARCHIVE" fish > "$FISH_DIR/usr/bin/fish"
        chmod 0755 "$FISH_DIR/usr/bin/fish"
    else
        tar -xf "$FISH_ARCHIVE" -C "$FISH_DIR"
        for dir in bin lib share; do
            if [ -d "$FISH_DIR/$dir" ]; then
                mv "$FISH_DIR/$dir" "$FISH_DIR/usr/$dir"
            fi
        done
        FISH_BIN="$(find "$FISH_DIR" -maxdepth 4 \( -type f -o -type l \) -name fish -perm -0100 -print -quit)"
        test -n "$FISH_BIN"
        if [ "$FISH_BIN" != "$FISH_DIR/usr/bin/fish" ]; then
            install -Dm0755 "$FISH_BIN" "$FISH_DIR/usr/bin/fish"
            rm -f "$FISH_BIN"
        fi
    fi
fi
[ -x "$FISH_DIR/usr/bin/fish" ] || { echo "fish payload binary not found" >&2; exit 1; }

if ! command -v zig >/dev/null 2>&1 || ! zig version | grep -qx "$GHOSTTY_ZIG_VERSION"; then
    ZIG_ARCHIVE="$SRC/zig-x86_64-linux-$GHOSTTY_ZIG_VERSION.tar.xz"
    if [ ! -f "$ZIG_ARCHIVE" ]; then
        curl -fsSL "https://ziglang.org/download/$GHOSTTY_ZIG_VERSION/zig-x86_64-linux-$GHOSTTY_ZIG_VERSION.tar.xz" -o "$ZIG_ARCHIVE"
    fi
    tar -xf "$ZIG_ARCHIVE" -C "$SRC"
    export PATH="$SRC/zig-x86_64-linux-$GHOSTTY_ZIG_VERSION:$PATH"
fi

for tool in cargo meson ninja zig cmake pkg-config ldd; do
    command -v "$tool" >/dev/null || { echo "missing required build tool: $tool" >&2; exit 1; }
done

(
    cd "$SRC/wayland"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Dtests=false -Ddocumentation=false
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)

export PKG_CONFIG_SYSROOT_DIR="$OUT"
export PKG_CONFIG_PATH="$OUT/usr/lib/pkgconfig:$OUT/usr/share/pkgconfig:$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"

(
    cd "$SRC/wayland-protocols"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Dtests=false
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)

export PKG_CONFIG_SYSROOT_DIR="$OUT"
export PKG_CONFIG_PATH="$OUT/usr/lib/pkgconfig:$OUT/usr/share/pkgconfig:$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"

# wlroots also needs host-provided development dependencies (libdrm,
# pixman, xkbcommon and the Smithay DRM metadata libraries). Expose their
# headers/pkg-config metadata through the staging sysroot only for this build,
# then remove the build-only files.
for dep in libdrm pixman-1 xkbcommon; do
    if [ -d "/usr/include/$dep" ]; then
        mkdir -p "$OUT/usr/include"
        ln -sfn "/usr/include/$dep" "$OUT/usr/include/$dep"
    fi
    pc="$(pkg-config --variable=pcfiledir "$dep")/$dep.pc"
    if [ -f "$pc" ]; then
        mkdir -p "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
        cp -f "$pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    fi
done
if [ -n "${LUNA_NIRI_DEV_ROOT:-}" ]; then
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig" "$OUT/usr/lib/x86_64-linux-gnu" "$OUT/usr/share/hwdata"
    if [ -f "${LUNA_NIRI_DEV_ROOT}/usr/share/hwdata/pnp.ids" ]; then
        cp -f "${LUNA_NIRI_DEV_ROOT}/usr/share/hwdata/pnp.ids" "$OUT/usr/share/hwdata/pnp.ids"
    else
        echo "missing hwdata pnp.ids in Niri development root" >&2
        exit 1
    fi
    for dep in libdisplay-info libliftoff; do
        pc="${LUNA_NIRI_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/${dep}.pc"
        if [ -f "$pc" ]; then
            cp -f "$pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
        fi
    done
    if [ -d "${LUNA_NIRI_DEV_ROOT}/usr/include/libdisplay-info" ]; then
        ln -sfn "${LUNA_NIRI_DEV_ROOT}/usr/include/libdisplay-info" "$OUT/usr/include/libdisplay-info"
    fi
    if [ -f "${LUNA_NIRI_DEV_ROOT}/usr/include/libliftoff.h" ]; then
        ln -sfn "${LUNA_NIRI_DEV_ROOT}/usr/include/libliftoff.h" "$OUT/usr/include/libliftoff.h"
    fi
    for lib in display-info liftoff; do
        found=0
        for src in "${LUNA_NIRI_DEV_ROOT}/usr/lib/x86_64-linux-gnu/lib${lib}.so"*; do
            [ -e "$src" ] || continue
            cp -a "$src" "$OUT/usr/lib/x86_64-linux-gnu/"
            found=1
        done
        [ "$found" -eq 1 ] || { echo "missing Niri development library: lib${lib}" >&2; exit 1; }
    done
fi
(
# hwdata's pkgdatadir must remain a host source path: wlroots embeds pnp.ids
# into generated source during the build, so applying DESTDIR as a pkg-config
# sysroot would incorrectly prepend the staging root to that source path.
unset PKG_CONFIG_SYSROOT_DIR
export CFLAGS="-I$OUT/usr/include"
export CXXFLAGS="-I$OUT/usr/include"
export LDFLAGS="-L$OUT/usr/lib/x86_64-linux-gnu"
    cd "$SRC/wlroots"
    rm -rf build-luna-${MESON_BUILD_SUFFIX}
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Dxwayland=disabled -Dexamples=false
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)
for dep in libdrm pixman-1 xkbcommon; do
    rm -f "$OUT/usr/include/$dep"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/$dep.pc"
done
unset PKG_CONFIG
unset LUNA_WLROOTS_HW_DATA
# WirePlumber needs native GLib helper tools such as glib-mkenums.
# Build it against the host development environment; only its installed
# runtime payload is placed into the Luna desktop root. Some CI hosts carry
# PipeWire runtime libraries without development headers, so an unpacked
# development root may be supplied with LUNA_PIPEWIRE_DEV_ROOT.
unset PKG_CONFIG_SYSROOT_DIR
unset PKG_CONFIG_PATH
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    PIPEWIRE_PKG_CONFIG_DIR="$SRC/pipewire-pkgconfig"
    rm -rf "$PIPEWIRE_PKG_CONFIG_DIR"
    mkdir -p "$PIPEWIRE_PKG_CONFIG_DIR"
    for pc in libpipewire-0.3 libspa-0.2; do
        src_pc="${LUNA_PIPEWIRE_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/${pc}.pc"
        test -f "$src_pc" || { echo "missing PipeWire pkg-config file: $src_pc" >&2; exit 1; }
        sed "s#^prefix=/usr\$#prefix=${LUNA_PIPEWIRE_DEV_ROOT}/usr#" "$src_pc" > "$PIPEWIRE_PKG_CONFIG_DIR/${pc}.pc"
    done
    export PKG_CONFIG_PATH="$PIPEWIRE_PKG_CONFIG_DIR:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
    export LDFLAGS="-L${LUNA_PIPEWIRE_DEV_ROOT}/usr/lib/x86_64-linux-gnu${LDFLAGS:+ $LDFLAGS}"
fi
(
    cd "$SRC/wireplumber"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)
unset CFLAGS
unset CPPFLAGS
unset BINDGEN_EXTRA_CLANG_ARGS
unset LDFLAGS
export PKG_CONFIG_SYSROOT_DIR="$OUT"
export PKG_CONFIG_PATH="$OUT/usr/lib/pkgconfig:$OUT/usr/share/pkgconfig:$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
if [ -n "${LUNA_NOCTALIA_DEV_ROOT:-}" ]; then
    export CMAKE_PREFIX_PATH="$OUT/usr${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"
fi
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    # niri builds PipeWire Rust bindings too. Expose the same development
    # headers/pkg-config metadata through the Luna staging sysroot for Cargo
    # and bindgen, then remove them before the desktop payload is finalized.
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
    ln -sfn "${LUNA_PIPEWIRE_DEV_ROOT}/usr/include/pipewire-0.3" "$OUT/usr/include/pipewire-0.3"
    ln -sfn "${LUNA_PIPEWIRE_DEV_ROOT}/usr/include/spa-0.2" "$OUT/usr/include/spa-0.2"
    cp -f "${LUNA_PIPEWIRE_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    cp -f "${LUNA_PIPEWIRE_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
fi
if [ -n "${LUNA_NIRI_DEV_ROOT:-}" ]; then
    # niri links these libraries while building. The development root provides
    # static archives and pkg-config metadata, but not the linker-facing .so
    # names, so stage the host shared libraries into the build sysroot. They
    # are also valid runtime payload and are retained for bundle_elf_deps().
    mkdir -p "$OUT/usr/lib/x86_64-linux-gnu"
    for lib in display-info drm input seat udev pixman-1 xkbcommon; do
        found=0
        for src in /usr/lib/x86_64-linux-gnu/lib${lib}.so*; do
            [ -e "$src" ] || continue
            cp -a "$src" "$OUT/usr/lib/x86_64-linux-gnu/"
            found=1
        done
        [ "$found" -eq 1 ] || { echo "missing host runtime library: lib${lib}" >&2; exit 1; }
    done
    # niri's Rust linker looks for the unversioned -lNAME development soname.
    for lib in display-info drm input seat udev pixman-1 xkbcommon; do
        so=$(find /usr/lib/x86_64-linux-gnu -maxdepth 1 -type f -name "lib${lib}.so.*" | sort -V | tail -n1)
        [ -n "$so" ] || continue
        ln -sfn "$(basename "$so")" "$OUT/usr/lib/x86_64-linux-gnu/lib${lib}.so"
    done
    # niri's Smithay backends use display-info, DRM, libinput, libseat,
    # libudev, pixman and xkbcommon development metadata and headers.
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
    while IFS= read -r -d '' inc; do
        name="$(basename "$inc")"
        ln -sfn "$inc" "$OUT/usr/include/$name"
    done < <(find "$LUNA_NIRI_DEV_ROOT/usr/include" -mindepth 1 -maxdepth 1 -type d -print0)
    cp -f "$LUNA_NIRI_DEV_ROOT"/usr/lib/x86_64-linux-gnu/pkgconfig/*.pc "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
fi

if [ -n "${LUNA_SDBUS_DEV_ROOT:-}" ]; then
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
    ln -sfn "${LUNA_SDBUS_DEV_ROOT}/usr/include/sdbus-c++" "$OUT/usr/include/sdbus-c++"
    cp -f "${LUNA_SDBUS_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/sdbus-c++.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    for lib in /usr/lib/x86_64-linux-gnu/libsdbus-c++.so*; do
        [ -e "$lib" ] || continue
        cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
    ln -sfn "$(basename "$(readlink -f /usr/lib/x86_64-linux-gnu/libsdbus-c++.so.2)")" "$OUT/usr/lib/x86_64-linux-gnu/libsdbus-c++.so"
fi
if [ -n "${LUNA_LIBRSVG_DEV_ROOT:-}" ]; then
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"
    if [ -d "${LUNA_LIBRSVG_DEV_ROOT}/usr/include/librsvg-2.0" ]; then
        ln -sfn "${LUNA_LIBRSVG_DEV_ROOT}/usr/include/librsvg-2.0" "$OUT/usr/include/librsvg-2.0"
    fi
    cp -f "${LUNA_LIBRSVG_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/librsvg-2.0.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    for lib in /usr/lib/x86_64-linux-gnu/librsvg-2.so*; do
        [ -e "$lib" ] || continue
        cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
    ln -sfn "$(basename "$(readlink -f /usr/lib/x86_64-linux-gnu/librsvg-2.so.2)")" "$OUT/usr/lib/x86_64-linux-gnu/librsvg-2.so"
fi
if [ -n "${LUNA_NOCTALIA_DEV_ROOT:-}" ]; then
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig" "$OUT/usr/share/pkgconfig"
    # Merge Noctalia's development headers without replacing include roots
    # already supplied by another native-development source. The staged roots
    # are build inputs only; none of these development headers belong in the
    # final System Image.
    while IFS= read -r -d '' inc; do
        name="$(basename "$inc")"
        dest="$OUT/usr/include/$name"
        if [ -e "$dest" ] || [ -L "$dest" ]; then
            continue
        fi
        cp -a "$inc" "$dest"
    done < <(find "${LUNA_NOCTALIA_DEV_ROOT}/usr/include" -mindepth 1 -maxdepth 1 -print0)
    if [ -n "${LUNA_GMP_DEV_ROOT:-}" ]; then
        mkdir -p "$OUT/usr/include"
        install -Dm0644 "${LUNA_GMP_DEV_ROOT}/usr/include/x86_64-linux-gnu/gmp.h" "$OUT/usr/include/gmp.h"
    fi
    if [ -n "${LUNA_MPFR_DEV_ROOT:-}" ]; then
        install -Dm0644 "${LUNA_MPFR_DEV_ROOT}/usr/include/mpfr.h" "$OUT/usr/include/mpfr.h"
    fi
    for lib in /usr/lib/x86_64-linux-gnu/libcurl.so* /usr/lib/x86_64-linux-gnu/libqalculate.so*; do
        [ -e "$lib" ] || continue
        cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
    ln -sfn "$(basename "$(readlink -f /usr/lib/x86_64-linux-gnu/libcurl.so.4)")" "$OUT/usr/lib/x86_64-linux-gnu/libcurl.so"
    ln -sfn "$(basename "$(readlink -f /usr/lib/x86_64-linux-gnu/libqalculate.so.23)")" "$OUT/usr/lib/x86_64-linux-gnu/libqalculate.so"
    # Keep Noctalia's direct development metadata, but do not force the build
    # to resolve optional curl/qalculate transitive development packages that
    # are not part of the Luna desktop staging root. Their shared libraries
    # carry their runtime dependencies themselves.
    for pc in libcurl libqalculate; do
        src_pc="${LUNA_NOCTALIA_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/${pc}.pc"
        sed -e '/^Requires:/d' -e '/^Requires.private:/d' "$src_pc" > "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/${pc}.pc"
    done
    cp -f "${LUNA_NOCTALIA_DEV_ROOT}/usr/share/pkgconfig/nlohmann_json.pc" "$OUT/usr/share/pkgconfig/"
    # Several Noctalia sources include GLib family headers directly. Keep the
    # host development include roots visible only for this native build.
    ln -sfn /usr/include/glib-2.0 "$OUT/usr/include/glib-2.0"
    mkdir -p "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0"
    ln -sfn /usr/lib/x86_64-linux-gnu/glib-2.0/include "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
    if [ -d /usr/include/cairo ]; then
        ln -sfn /usr/include/cairo "$OUT/usr/include/cairo"
    fi
    if [ -d /usr/include/freetype2 ]; then
        ln -sfn /usr/include/freetype2 "$OUT/usr/include/freetype2"
    fi
    if [ -d /usr/include/pango-1.0 ]; then
        ln -sfn /usr/include/pango-1.0 "$OUT/usr/include/pango-1.0"
    fi
    if [ -d /usr/include/harfbuzz ]; then
        ln -sfn /usr/include/harfbuzz "$OUT/usr/include/harfbuzz"
    fi
    for inc in polkit-1 gio-unix-2.0 libmount blkid fribidi sysprof-6 gdk-pixbuf-2.0 glycin-2 libsecret-1 p11-kit-1 webp opus; do
        if [ -d "/usr/include/$inc" ]; then
            ln -sfn "/usr/include/$inc" "$OUT/usr/include/$inc"
        fi
    done
fi

(
    cd "$SRC/niri"
    CARGO_TARGET_DIR="$NIRI_TARGET_DIR" cargo build --release
)
install -Dm0755 "$NIRI_TARGET_DIR/release/niri" "$OUT/usr/bin/niri"
install -Dm0755 "$SRC/niri/resources/niri-session" "$OUT/usr/share/niri/upstream-niri-session"
install -Dm0644 "$SRC/niri/resources/niri.desktop" "$OUT/usr/share/wayland-sessions/niri.desktop"
install -Dm0644 "$SRC/niri/resources/niri-portals.conf" "$OUT/usr/share/xdg-desktop-portal/niri-portals.conf"
# libinput uses runtime quirks data from /usr/share; without it wlroots cannot
# initialize the input backend inside the self-contained Luna System Image.
if [ -d /usr/share/libinput ]; then
    mkdir -p "$OUT/usr/share/libinput"
    cp -a /usr/share/libinput/. "$OUT/usr/share/libinput/"
fi
# libinput uses udev for device enumeration and ID_INPUT/ID_SEAT tagging.
# Luna does not use systemd as its service manager, so only the standalone
# udev daemon binary and the small set of input/seat rules are staged here.
install -Dm0755 /usr/bin/udevadm "$OUT/usr/bin/systemd-udevd"
install -Dm0755 /usr/bin/udevadm "$OUT/usr/bin/udevadm"
mkdir -p "$OUT/usr/lib/udev/rules.d"
for rule in 50-udev-default.rules 60-input-id.rules 71-seat.rules 73-seat-late.rules; do
    if [ -f "/usr/lib/udev/rules.d/$rule" ]; then
        install -Dm0644 "/usr/lib/udev/rules.d/$rule" "$OUT/usr/lib/udev/rules.d/$rule"
    fi
done
if [ -f /usr/lib/udev/hwdb.bin ]; then
    install -Dm0444 /usr/lib/udev/hwdb.bin "$OUT/usr/lib/udev/hwdb.bin"
fi
if [ -n "${LUNA_NIRI_DEV_ROOT:-}" ]; then
    while IFS= read -r -d '' inc; do
        rm -rf "$OUT/usr/include/$(basename "$inc")"
    done < <(find "$LUNA_NIRI_DEV_ROOT/usr/include" -mindepth 1 -maxdepth 1 -type d -print0)
    find "$LUNA_NIRI_DEV_ROOT/usr/lib/x86_64-linux-gnu/pkgconfig" -maxdepth 1 -type f -name '*.pc' -print0 \
        | while IFS= read -r -d '' pc; do rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/$(basename "$pc")"; done
fi

(
    cd "$SRC/noctalia"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Dnative_optimizations=false -Djemalloc=disabled -Dtests=disabled
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)

# Luna owns authentication and graphical session creation. Noctalia's optional
# greeter integration is a separate login-provider product and must not enter
# the Luna desktop payload, even when upstream installs it by default.
rm -rf "$OUT/usr/share/noctalia-greeter" "$OUT/var/lib/noctalia-greeter"
rm -f "$OUT/usr/bin/greetd" "$OUT/usr/bin/noctalia-greeter" "$OUT/usr/bin/noctalia-greeter-apply-appearance" "$OUT/usr/bin/noctalia-greeter-compositor" "$OUT/usr/bin/noctalia-greeter-print-greetd-config" "$OUT/usr/bin/noctalia-greeter-session" "$OUT/usr/lib/tmpfiles.d/noctalia-greeter.conf" "$OUT/usr/share/polkit-1/actions/org.noctalia.greeter.apply-appearance.policy" "$OUT/etc/pam.d/greetd" "$OUT/etc/pam.d/greetd-greeter"
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/pipewire-0.3" "$OUT/usr/include/spa-0.2"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc"
fi
if [ -d /usr/include/pixman-1 ]; then
    ln -sfn /usr/include/pixman-1 "$OUT/usr/include/pixman-1"
fi

if [ -n "${LUNA_SDBUS_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/sdbus-c++" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/sdbus-c++.pc"
fi
if [ -n "${LUNA_LIBRSVG_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/librsvg-2.0" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/librsvg-2.0.pc"
fi
rm -rf "$OUT/usr/include/pixman-1"
if [ -n "${LUNA_NOCTALIA_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/nlohmann" "$OUT/usr/include/libqalculate"
    rm -rf "$OUT/usr/include/cairo" "$OUT/usr/include/freetype2" "$OUT/usr/include/pango-1.0" "$OUT/usr/include/harfbuzz"
    rm -rf "$OUT/usr/include/curl" "$OUT/usr/include/glib-2.0" "$OUT/usr/include/gmp.h" "$OUT/usr/include/mpfr.h"
    rm -rf "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
    for inc in polkit-1 gio-unix-2.0 libmount blkid fribidi sysprof-6 gdk-pixbuf-2.0 glycin-2 libsecret-1 p11-kit-1 webp opus; do
        rm -rf "$OUT/usr/include/$inc"
    done
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libcurl.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libqalculate.pc" "$OUT/usr/share/pkgconfig/nlohmann_json.pc"
fi
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/pipewire-0.3" "$OUT/usr/include/spa-0.2"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc"
fi

# Ghostty's GTK build uses libadwaita's Blueprint compiler and gtk4-layer-shell.
# Expose the host development headers and linker-facing libraries for the
# native build; runtime dependencies remain in the root for closure collection.
# D-Bus daemon needs its system/session configuration at the logical
# /usr/share/dbus-1 path. The service files alone are insufficient.
mkdir -p "$OUT/usr/share/dbus-1"
for cfg in system.conf session.conf; do
    if [ -f "/usr/share/dbus-1/$cfg" ]; then
        cp -f "/usr/share/dbus-1/$cfg" "$OUT/usr/share/dbus-1/$cfg"
    fi
done
# Luna supervises system services itself, so D-Bus must not depend on the
# distro-specific setuid activation helper or external service manager.
if [ -f "$OUT/usr/share/dbus-1/system.conf" ]; then
    sed -i '/<standard_system_servicedirs\/>/d; /<servicehelper>/d' "$OUT/usr/share/dbus-1/system.conf"
fi
if [ -d /usr/share/dbus-1/system.d ]; then
    mkdir -p "$OUT/usr/share/dbus-1/system.d"
    cp -a /usr/share/dbus-1/system.d/. "$OUT/usr/share/dbus-1/system.d/"
fi
# Polkit actions are desktop policy resources and are consumed through the
# system D-Bus namespace; keep them with dbus-daemon's application resources.
if [ -d /usr/share/polkit-1 ]; then
    mkdir -p "$OUT/usr/share/polkit-1"
    cp -a /usr/share/polkit-1/. "$OUT/usr/share/polkit-1/"
fi

if [ -n "${LUNA_GTK4_LAYER_SHELL_DEV_ROOT:-}" ]; then
    mkdir -p "$OUT/usr/include" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig" "$OUT/usr/lib/x86_64-linux-gnu"
    ln -sfn "${LUNA_GTK4_LAYER_SHELL_DEV_ROOT}/usr/include/gtk4-layer-shell" "$OUT/usr/include/gtk4-layer-shell"
    cp -f "${LUNA_GTK4_LAYER_SHELL_DEV_ROOT}/usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    for lib in "${LUNA_GTK4_LAYER_SHELL_DEV_ROOT}"/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so* /usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so*; do
        [ -e "$lib" ] || continue
        cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
else
    if [ -d /usr/include/gtk4-layer-shell ]; then
        ln -sfn /usr/include/gtk4-layer-shell "$OUT/usr/include/gtk4-layer-shell"
    fi
    if [ -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc ]; then
        cp -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    fi
    for lib in /usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so*; do
        [ -e "$lib" ] || continue
        cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
fi
layer_shell_so="$(find "$OUT/usr/lib/x86_64-linux-gnu" -maxdepth 1 -type f -name 'libgtk4-layer-shell.so.*' -print 2>/dev/null | sort -V | tail -n1)"
[ -z "$layer_shell_so" ] || ln -sfn "$(basename "$layer_shell_so")" "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so"
[ -z "$layer_shell_so" ] || ln -sfn "$(basename "$layer_shell_so")" "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell-0.so"
if [ -d /usr/include/libadwaita-1 ]; then
    ln -sfn /usr/include/libadwaita-1 "$OUT/usr/include/libadwaita-1"
fi
if [ -f /usr/lib/x86_64-linux-gnu/pkgconfig/libadwaita-1.pc ]; then
    cp -f /usr/lib/x86_64-linux-gnu/pkgconfig/libadwaita-1.pc "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
fi
mkdir -p "$OUT/usr/lib/x86_64-linux-gnu"
for lib in /usr/lib/x86_64-linux-gnu/libadwaita-1.so*; do
    [ -e "$lib" ] || continue
    cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
done
adwaita_so="$(find /usr/lib/x86_64-linux-gnu -maxdepth 1 -type f -name 'libadwaita-1.so.*' | sort -V | tail -n1)"
[ -z "$adwaita_so" ] || ln -sfn "$(basename "$adwaita_so")" "$OUT/usr/lib/x86_64-linux-gnu/libadwaita-1.so"
if [ -d /usr/include/gtk-4.0 ]; then
    ln -sfn /usr/include/gtk-4.0 "$OUT/usr/include/gtk-4.0"
fi
if [ -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4.pc ]; then
    cp -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4.pc "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
fi
for lib in /usr/lib/x86_64-linux-gnu/libgtk-4.so*; do
    [ -e "$lib" ] || continue
    cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
done
gtk4_so="$(find /usr/lib/x86_64-linux-gnu -maxdepth 1 -type f -name 'libgtk-4.so.*' | sort -V | tail -n1)"
[ -z "$gtk4_so" ] || ln -sfn "$(basename "$gtk4_so")" "$OUT/usr/lib/x86_64-linux-gnu/libgtk-4.so"
for inc in glib-2.0 gio-unix-2.0 libmount blkid sysprof-6 fribidi gdk-pixbuf-2.0 glycin-2 pango-1.0 harfbuzz cairo freetype2 libpng16 pixman-1 graphene-1.0 appstream; do
    if [ -d "/usr/include/$inc" ]; then
        ln -sfn "/usr/include/$inc" "$OUT/usr/include/$inc"
    fi
done
mkdir -p "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0"
if [ -d /usr/lib/x86_64-linux-gnu/glib-2.0/include ]; then
    ln -sfn /usr/lib/x86_64-linux-gnu/glib-2.0/include "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
fi
mkdir -p "$OUT/usr/lib/x86_64-linux-gnu/graphene-1.0"
if [ -d /usr/lib/x86_64-linux-gnu/graphene-1.0/include ]; then
    ln -sfn /usr/lib/x86_64-linux-gnu/graphene-1.0/include "$OUT/usr/lib/x86_64-linux-gnu/graphene-1.0/include"
fi
for pc in glib-2.0 gobject-2.0 gio-2.0 gtk4; do
    if [ -f "/usr/lib/x86_64-linux-gnu/pkgconfig/$pc.pc" ]; then
        cp -f "/usr/lib/x86_64-linux-gnu/pkgconfig/$pc.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
    fi
done

(
    cd "$SRC/ghostty"
    if ! command -v blueprint-compiler >/dev/null 2>&1; then
        echo "blueprint-compiler is required to build Ghostty" >&2
        exit 1
    fi
    # Use the staged gtk4-layer-shell as a system integration while keeping
    # Ghostty's other Zig dependencies on their normal package mechanism.
    export C_INCLUDE_PATH="$OUT/usr/include/gtk4-layer-shell:$OUT/usr/include/gtk-4.0:$OUT/usr/include"
    export LIBRARY_PATH="$OUT/usr/lib/x86_64-linux-gnu"
    zig build -fsys=gtk4-layer-shell -Doptimize=ReleaseFast -Dapp-runtime=gtk -Di18n=false -p "$OUT/usr"
)

# Remove development-only staging before assembling the runtime closure.
rm -rf "$OUT/usr/include/gtk4-layer-shell"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so" "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell-0.so"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libadwaita-1.so"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libgtk-4.so"
rm -rf "$OUT/usr/include/libadwaita-1" "$OUT/usr/include/gtk-4.0"
for inc in glib-2.0 gio-unix-2.0 libmount blkid sysprof-6 fribidi gdk-pixbuf-2.0 glycin-2 pango-1.0 harfbuzz cairo freetype2 libpng16 pixman-1 graphene-1.0 appstream; do
    rm -rf "$OUT/usr/include/$inc"
done
rm -rf "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
rm -rf "$OUT/usr/lib/x86_64-linux-gnu/graphene-1.0/include"
for pc in glib-2.0 gobject-2.0 gio-2.0 gtk4 libadwaita-1; do
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/$pc.pc"
done

cp -a "$FISH_DIR"/. "$OUT/"
install -Dm0755 "$(command -v bash)" "$OUT/usr/bin/bash"
ln -sfn /usr/bin/bash "$OUT/usr/bin/sh"
ln -sfn /usr/bin/fish "$OUT/usr/bin/luna-shell"

for tool in dbus-daemon dbus-send; do
    if [ -x "/usr/bin/$tool" ]; then
        install -Dm0755 "/usr/bin/$tool" "$OUT/usr/bin/$tool"
    fi
done

cat > "$OUT/etc/luna/niri-config.kdl" <<'EOF'
input {
    keyboard {
        xkb {
            layout "us"
        }
    }
}

layout {
    gaps 8
    center-focused-column "never"
}

spawn-at-startup "/usr/bin/noctalia"

binds {
    Mod+Return repeat=false hotkey-overlay-title="Open Ghostty" { spawn "/usr/bin/ghostty"; }
    Mod+Shift+Slash { show-hotkey-overlay; }
    Mod+Q { close-window; }
    Mod+Shift+E repeat=false { quit skip-confirmation=true; }
}

hotkey-overlay {
    skip-at-startup
}

prefer-no-csd
EOF

# Project Luna visual identity is optional until the branding asset is present.
# The runtime desktop must not fail merely because the source tree has no icon.
if [ -f "$REPO_ROOT/assets/icons/luna.svg" ]; then
    install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/dev.projectluna.Luna.svg"
    install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/dev.projectluna.Files.svg"
    install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/luna.svg"
fi

cat > "$OUT/usr/share/applications/ghostty.desktop" <<'EOF'
[Desktop Entry]
Name=Ghostty
Comment=Fast, native terminal emulator
Exec=/usr/bin/ghostty
Icon=utilities-terminal
Terminal=false
Type=Application
Categories=System;TerminalEmulator;
EOF

cat > "$OUT/usr/share/applications/luna-files.desktop" <<'EOF'
[Desktop Entry]
Name=Luna Files
Comment=Project Luna file manager
Exec=/usr/bin/luna-files
Icon=dev.projectluna.Files
Terminal=false
Type=Application
Categories=Utility;FileManager;GTK;
StartupNotify=true
EOF

cat > "$OUT/usr/share/applications/luna.desktop" <<'EOF'
[Desktop Entry]
Name=Project Luna
Comment=Project Luna desktop
Exec=/usr/bin/luna-files
Icon=dev.projectluna.Luna
Terminal=false
Type=Application
Categories=System;Utility;
StartupNotify=true
EOF

cat > "$OUT/usr/share/wayland-sessions/luna.desktop" <<'EOF'
[Desktop Entry]
Name=Project Luna
Comment=Project Luna graphical desktop
Exec=/usr/bin/luna-user-session --handoff
Icon=dev.projectluna.Luna
Type=Application
DesktopNames=niri;
EOF

cat > "$OUT/etc/profile" <<'EOF'
export PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export XDG_SESSION_TYPE=wayland
export MOZ_ENABLE_WAYLAND=1
export QT_QPA_PLATFORM=wayland
export SDL_VIDEODRIVER=wayland
export SHELL=/usr/bin/fish
EOF

cat > "$OUT/etc/shells" <<'EOF'
/usr/bin/fish
/usr/bin/bash
/usr/bin/sh
EOF

cat > "$OUT/etc/luna/desktop.toml" <<EOF
[desktop]
compositor = "niri"
shell = "noctalia"
terminal = "ghostty"
interactive_shell = "fish"
compatibility_shell = "bash"
posix_shell = "sh"
icon = "dev.projectluna.Luna"
file_manager_icon = "dev.projectluna.Files"

[versions]
niri = "$NIRI_TAG"
noctalia = "$NOCTALIA_TAG"
ghostty = "$GHOSTTY_TAG"
fish = "$FISH_VERSION"
wayland = "$WAYLAND_VERSION"
wayland_protocols = "$WAYLAND_PROTOCOLS_VERSION"
wlroots = "$WLROOTS_VERSION"
wireplumber = "$WIREPLUMBER_VERSION"
EOF

cargo build --release -p luna-user-session --bin luna-user-session
install -Dm0755 "$REPO_ROOT/target/release/luna-user-session" "$OUT/usr/bin/luna-user-session"

bundle_elf_deps() {
    local root="$1"
    local pass=0
    while [ "$pass" -lt 8 ]; do
        pass=$((pass + 1))
        local changed=0
        while IFS= read -r -d '' elf; do
            file "$elf" | grep -q 'ELF' || continue
            while IFS= read -r dep; do
                case "$dep" in
                    /lib/*|/lib64/*|/usr/lib/*)
                        [ -e "$dep" ] || continue
                        local rel="${dep#/}"
                        local dst="$root/$rel"
                        if [ ! -e "$dst" ]; then
                            mkdir -p "$(dirname "$dst")"
                            cp -a "$dep" "$dst"
                            changed=1
                        fi
                        ;;
                esac
            done < <(ldd "$elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib64|lib|usr\/lib)/ {print $1}')
        done < <(find "$root" -type f -perm -0100 -print0)
        if [ "$changed" -eq 0 ]; then
            break
        fi
    done
}

bundle_elf_deps "$OUT"

# GLVND selects Mesa EGL/DRI components through dynamic discovery, so the
# normal executable dependency walk cannot see them. Stage the Mesa loader,
# GBM backend and the QEMU virtio DRI driver explicitly, then resolve their
# ELF dependencies into the same runtime closure.
mkdir -p "$OUT/usr/share/glvnd/egl_vendor.d" "$OUT/usr/lib/x86_64-linux-gnu/dri" "$OUT/usr/lib/x86_64-linux-gnu/gbm"
if [ -f /usr/share/glvnd/egl_vendor.d/50_mesa.json ]; then
    cp -a /usr/share/glvnd/egl_vendor.d/50_mesa.json "$OUT/usr/share/glvnd/egl_vendor.d/"
fi
for lib in /usr/lib/x86_64-linux-gnu/libEGL_mesa.so* /usr/lib/x86_64-linux-gnu/libGLX_mesa.so* /usr/lib/x86_64-linux-gnu/libgallium-*.so*; do
    [ -e "$lib" ] || continue
    cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
done
for lib in /usr/lib/x86_64-linux-gnu/dri/virtio_gpu_dri.so /usr/lib/x86_64-linux-gnu/dri/swrast_dri.so /usr/lib/x86_64-linux-gnu/dri/kms_swrast_dri.so /usr/lib/x86_64-linux-gnu/gbm/dri_gbm.so; do
    [ -e "$lib" ] || continue
    mkdir -p "$OUT/$(dirname "${lib#/}")"
    cp -a "$lib" "$OUT/${lib#/}"
done
# Debian/Ubuntu ship the Mesa DRI frontends above as symlinks to the common
# Gallium loader, which must be staged explicitly because it is not an ELF
# DT_NEEDED dependency of the symlink itself.
for lib in /usr/lib/x86_64-linux-gnu/dri/libdril_dri.so*; do
    [ -e "$lib" ] || continue
    cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/dri/"
done

bundle_elf_runtime_deps() {
    local root="$1"
    local pass=0
    while [ "$pass" -lt 8 ]; do
        pass=$((pass + 1))
        local changed=0
        while IFS= read -r -d '' elf; do
            file "$elf" | grep -q 'ELF' || continue
            while IFS= read -r dep; do
                case "$dep" in
                    /lib/*|/lib64/*|/usr/lib/*)
                        [ -e "$dep" ] || continue
                        local rel="${dep#/}"
                        local dst="$root/$rel"
                        if [ ! -e "$dst" ]; then
                            mkdir -p "$(dirname "$dst")"
                            cp -a "$dep" "$dst"
                            changed=1
                        fi
                        ;;
                esac
            done < <(ldd "$elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib64|lib|usr\/lib)/ {print $1}')
        done < <(find "$root/usr/lib" -type f \( -name '*.so' -o -name '*.so.*' -o -name '*.dri.so' \) -print0)
        if [ "$changed" -eq 0 ]; then
            break
        fi
    done
}

bundle_elf_runtime_deps "$OUT"

# Mesa's GLVND vendor library has optional X11/DRM monitoring dependencies
# that are not always reported by the native link closure in this staging
# setup. They must still be present for libEGL_mesa to load inside Luna.
for lib in \
    libxcb-dri3.so.0 libxcb-present.so.0 libxcb-randr.so.0 libxcb-xfixes.so.0 \
    libxcb-sync.so.1 libxshmfence.so.1 libdrm_amdgpu.so.1 libdrm_intel.so.1 \
    libsensors.so.5 libedit.so.2 libpciaccess.so.0 libbsd.so.0 libmd.so.0; do
    for src in /usr/lib/x86_64-linux-gnu/${lib}*; do
        [ -e "$src" ] || continue
        cp -a "$src" "$OUT/usr/lib/x86_64-linux-gnu/"
    done
done
bundle_elf_runtime_deps "$OUT"

ELF_LOADER="$(readlink -f /lib64/ld-linux-x86-64.so.2)"
[ -n "$ELF_LOADER" ] && [ -f "$ELF_LOADER" ] || { echo "ELF loader not found on build host" >&2; exit 1; }
install -Dm0755 "$ELF_LOADER" "$OUT/usr/lib/x86_64-linux-gnu/$(basename "$ELF_LOADER")"
mkdir -p "$OUT/lib64"
ln -sfn "../usr/lib/x86_64-linux-gnu/$(basename "$ELF_LOADER")" "$OUT/lib64/ld-linux-x86-64.so.2"

PAM_UNIX="$(find /usr/lib /lib -path '*/security/pam_unix.so' -print -quit)"
PAM_PERMIT="$(find /usr/lib /lib -path '*/security/pam_permit.so' -print -quit)"
[ -n "$PAM_UNIX" ] || { echo "pam_unix.so not found on CI host" >&2; exit 1; }
[ -n "$PAM_PERMIT" ] || { echo "pam_permit.so not found on CI host" >&2; exit 1; }
install -Dm0644 "$PAM_UNIX" "$OUT/${PAM_UNIX#/}"
install -Dm0644 "$PAM_PERMIT" "$OUT/${PAM_PERMIT#/}"
while IFS= read -r dep; do
    case "$dep" in
        /lib/*|/lib64/*|/usr/lib/*)
            rel="${dep#/}"
            [ -e "$OUT/$rel" ] || { mkdir -p "$(dirname "$OUT/$rel")"; cp -a "$dep" "$OUT/$rel"; }
            ;;
    esac
done < <(ldd "$PAM_UNIX" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib64|lib|usr\/lib)/ {print $1}')

cat > "$OUT/etc/luna/desktop-ready" <<'LUNA_DESKTOP_READY_EOF'
Project Luna graphical desktop payload
niri + Noctalia + Ghostty + fish + Luna Files
LUNA_DESKTOP_READY_EOF
