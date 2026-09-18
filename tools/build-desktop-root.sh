#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_DESKTOP_ROOT_OUT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"
MESON_BUILD_SUFFIX="$(basename "$OUT")"
NIRI_TARGET_DIR="$SRC/niri/target-${MESON_BUILD_SUFFIX}"

NIRI_TAG="${LUNA_NIRI_TAG:-v26.04}"
NOCTALIA_TAG="${LUNA_NOCTALIA_TAG:-v5.0.0-beta.8}"
NOCTALIA_GREETER_REF="${LUNA_NOCTALIA_GREETER_REF:-b4e668d4f8aada549d5c990c3a18458fae8be6b9}"
GREETD_REF="${LUNA_GREETD_REF:-0.10.3}"
GHOSTTY_TAG="${LUNA_GHOSTTY_TAG:-v1.3.1}"
GHOSTTY_ZIG_VERSION="${LUNA_GHOSTTY_ZIG_VERSION:-0.15.2}"
FISH_VERSION="${LUNA_FISH_VERSION:-4.8.1}"
WAYLAND_VERSION="${LUNA_WAYLAND_VERSION:-1.26.0}"
WAYLAND_PROTOCOLS_VERSION="${LUNA_WAYLAND_PROTOCOLS_VERSION:-1.49}"
WLROOTS_VERSION="${LUNA_WLROOTS_VERSION:-0.20.2}"
WIREPLUMBER_VERSION="${LUNA_WIREPLUMBER_VERSION:-0.5.13}"

mkdir -p "$OUT" "$SRC"
rm -rf "$OUT"
mkdir -p "$OUT/usr/bin" "$OUT/usr/lib" "$OUT/usr/share" "$OUT/etc/profile.d" "$OUT/etc/luna" "$OUT/etc/pam.d" "$OUT/var/lib/noctalia-greeter"

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
fetch_git https://github.com/noctalia-dev/noctalia-greeter.git "$NOCTALIA_GREETER_REF" "$SRC/noctalia-greeter"
fetch_git https://github.com/kennylevinsen/greetd.git "$GREETD_REF" "$SRC/greetd"
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
# pixman and xkbcommon). Expose their headers/pkg-config metadata through
# the staging sysroot only for this build, then remove the build-only files.
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
(
    cd "$SRC/wlroots"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Dxwayland=disabled -Dexamples=false
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)
for dep in libdrm pixman-1 xkbcommon; do
    rm -f "$OUT/usr/include/$dep"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/$dep.pc"
done
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

bash "$REPO_ROOT/tools/patch-noctalia-greeter.sh" "$SRC/noctalia-greeter/meson.build"

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
    cp -a "${LUNA_NOCTALIA_DEV_ROOT}/usr/include/." "$OUT/usr/include/"
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
if [ -n "${LUNA_NIRI_DEV_ROOT:-}" ]; then
    while IFS= read -r -d '' inc; do
        rm -f "$OUT/usr/include/$(basename "$inc")"
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
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    rm -f "$OUT/usr/include/pipewire-0.3" "$OUT/usr/include/spa-0.2"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc"
fi
if [ -d /usr/include/pixman-1 ]; then
    ln -sfn /usr/include/pixman-1 "$OUT/usr/include/pixman-1"
fi

(
    cd "$SRC/noctalia-greeter"
    meson setup build-luna-${MESON_BUILD_SUFFIX} --buildtype=release --prefix=/usr -Db_lto=true
    meson compile -C build-luna-${MESON_BUILD_SUFFIX} -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna-${MESON_BUILD_SUFFIX}
)
if [ -n "${LUNA_SDBUS_DEV_ROOT:-}" ]; then
    rm -f "$OUT/usr/include/sdbus-c++" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/sdbus-c++.pc"
fi
if [ -n "${LUNA_LIBRSVG_DEV_ROOT:-}" ]; then
    rm -f "$OUT/usr/include/librsvg-2.0" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/librsvg-2.0.pc"
fi
rm -f "$OUT/usr/include/pixman-1"
if [ -n "${LUNA_NOCTALIA_DEV_ROOT:-}" ]; then
    rm -rf "$OUT/usr/include/nlohmann" "$OUT/usr/include/libqalculate"
    rm -f "$OUT/usr/include/cairo" "$OUT/usr/include/freetype2" "$OUT/usr/include/pango-1.0" "$OUT/usr/include/harfbuzz"
    rm -f "$OUT/usr/include/curl" "$OUT/usr/include/glib-2.0" "$OUT/usr/include/gmp.h" "$OUT/usr/include/mpfr.h"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
    for inc in polkit-1 gio-unix-2.0 libmount blkid fribidi sysprof-6 gdk-pixbuf-2.0 glycin-2 libsecret-1 p11-kit-1 webp opus; do
        rm -f "$OUT/usr/include/$inc"
    done
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libcurl.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libqalculate.pc" "$OUT/usr/share/pkgconfig/nlohmann_json.pc"
fi
if [ -n "${LUNA_PIPEWIRE_DEV_ROOT:-}" ]; then
    rm -f "$OUT/usr/include/pipewire-0.3" "$OUT/usr/include/spa-0.2"
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc"
fi

(
    cd "$SRC/greetd"
    cargo build --release
)
install -Dm0755 "$SRC/greetd/target/release/greetd" "$OUT/usr/bin/greetd"

# Ghostty's GTK build uses libadwaita's Blueprint compiler and gtk4-layer-shell.
# Expose the host development headers and linker-facing libraries for the
# native build; runtime dependencies remain in the root for closure collection.
if [ -d /usr/include/gtk4-layer-shell ]; then
    ln -sfn /usr/include/gtk4-layer-shell "$OUT/usr/include/gtk4-layer-shell"
elif [ -d "${REPO_ROOT}/dist/tools/gtk4-layer-shell-dev/usr/include/gtk4-layer-shell" ]; then
    ln -sfn "${REPO_ROOT}/dist/tools/gtk4-layer-shell-dev/usr/include/gtk4-layer-shell" "$OUT/usr/include/gtk4-layer-shell"
fi
if [ -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc ]; then
    cp -f /usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
elif [ -f "${REPO_ROOT}/dist/tools/gtk4-layer-shell-dev/usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc" ]; then
    cp -f "${REPO_ROOT}/dist/tools/gtk4-layer-shell-dev/usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc" "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/"
fi
for lib in /usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so* "${REPO_ROOT}"/dist/tools/gtk4-layer-shell-dev/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so*; do
    [ -e "$lib" ] || continue
    cp -a "$lib" "$OUT/usr/lib/x86_64-linux-gnu/"
done
layer_shell_so="$(find /usr/lib/x86_64-linux-gnu "${REPO_ROOT}/dist/tools/gtk4-layer-shell-dev/usr/lib/x86_64-linux-gnu" -maxdepth 1 -type f -name 'libgtk4-layer-shell.so.*' 2>/dev/null | sort -V | tail -n1)"
[ -z "$layer_shell_so" ] || ln -sfn "$(basename "$layer_shell_so")" "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so"
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
    zig build -Doptimize=ReleaseFast -Dapp-runtime=gtk -Di18n=false -p "$OUT/usr"
)

# Remove development-only staging before assembling the runtime closure.
rm -f "$OUT/usr/include/gtk4-layer-shell"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/gtk4-layer-shell-0.pc"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libgtk4-layer-shell.so"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libadwaita-1.so"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/libgtk-4.so"
rm -f "$OUT/usr/include/libadwaita-1" "$OUT/usr/include/gtk-4.0"
for inc in glib-2.0 gio-unix-2.0 libmount blkid sysprof-6 fribidi gdk-pixbuf-2.0 glycin-2 pango-1.0 harfbuzz cairo freetype2 libpng16 pixman-1 graphene-1.0 appstream; do
    rm -f "$OUT/usr/include/$inc"
done
rm -f "$OUT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
rm -f "$OUT/usr/lib/x86_64-linux-gnu/graphene-1.0/include"
for pc in glib-2.0 gobject-2.0 gio-2.0 gtk4 libadwaita-1; do
    rm -f "$OUT/usr/lib/x86_64-linux-gnu/pkgconfig/$pc.pc"
done

cp -a "$FISH_DIR"/. "$OUT/"
install -Dm0755 "$(command -v bash)" "$OUT/usr/bin/bash"
ln -sfn /usr/bin/bash "$OUT/usr/bin/sh"
ln -sfn /usr/bin/fish "$OUT/usr/bin/luna-shell"

for tool in dbus-run-session dbus-daemon dbus-send; do
    if [ -x "/usr/bin/$tool" ]; then
        install -Dm0755 "/usr/bin/$tool" "$OUT/usr/bin/$tool"
    fi
done
if [ -x /usr/bin/setpriv ]; then
    install -Dm0755 /usr/bin/setpriv "$OUT/usr/bin/setpriv"
else
    echo "setpriv is required for non-root graphical sessions" >&2
    exit 1
fi

cat > "$OUT/usr/bin/niri-session" <<'LUNA_NIRI_SESSION_EOF'
#!/bin/sh
set -eu
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export MOZ_ENABLE_WAYLAND=1
export QT_QPA_PLATFORM=wayland
export SDL_VIDEODRIVER=wayland
export XDG_DATA_DIRS="/usr/local/share:/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
mkdir -p "$HOME/.config/niri"
if [ ! -e "$HOME/.config/niri/config.kdl" ]; then
    cp /etc/luna/niri-config.kdl "$HOME/.config/niri/config.kdl"
fi
if command -v dbus-run-session >/dev/null 2>&1; then
    exec dbus-run-session -- /usr/bin/niri --session
fi
exec /usr/bin/niri --session
LUNA_NIRI_SESSION_EOF
chmod 0755 "$OUT/usr/bin/niri-session"


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

# Project Luna visual identity. Use one vector icon everywhere so the desktop,
# file manager and launcher remain consistent without raster duplication.
install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/dev.projectluna.Luna.svg"
install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/dev.projectluna.Files.svg"
install -Dm0644 "$REPO_ROOT/assets/icons/luna.svg" "$OUT/usr/share/icons/hicolor/scalable/apps/luna.svg"

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

cat > "$OUT/var/lib/noctalia-greeter/greeter.toml" <<'EOF'
[session]
default = "Project Luna"

[auth]
allow_empty_password = false
request_timeout = 60
EOF

cat > "$OUT/etc/pam.d/greetd" <<'EOF'
auth required pam_unix.so
account required pam_unix.so
session required pam_unix.so
EOF

cat > "$OUT/etc/pam.d/greetd-greeter" <<'EOF'
account required pam_permit.so
session required pam_permit.so
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
noctalia_greeter = "$NOCTALIA_GREETER_REF"
greetd = "$GREETD_REF"
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
