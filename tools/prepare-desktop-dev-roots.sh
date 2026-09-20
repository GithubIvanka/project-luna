#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${LUNA_OUT_DIR:-${REPO_ROOT}/dist}"
PKG_CACHE="${LUNA_DEVPKG_CACHE:-${DIST}/.build/packages}"
DEV_ROOT="${LUNA_DESKTOP_DEV_ROOT_OUT:-${DIST}/.build/dev}"

command -v apt-get >/dev/null 2>&1 || {
    echo "apt-get is required to prepare the Alpha desktop development sysroot" >&2
    exit 1
}
command -v dpkg-deb >/dev/null 2>&1 || {
    echo "dpkg-deb is required to prepare the Alpha desktop development sysroot" >&2
    exit 1
}

COMMON_PACKAGES=(
    libwayland-dev
    wayland-protocols
    libegl-dev
    libgles-dev
    libfreetype-dev
    libfontconfig-dev
    libcairo2-dev
    libpango1.0-dev
    libharfbuzz-dev
    libxkbcommon-dev
    libglib2.0-dev
    libsecret-1-dev
    libsodium-dev
    libsdbus-c++-dev
    libpipewire-0.3-dev
    libspa-0.2-dev
    libwireplumber-0.5-dev
    libpam0g-dev
    libpolkit-agent-1-dev
    libpolkit-gobject-1-dev
    libcurl4-openssl-dev
    libwebp-dev
    libjxl-dev
    libsndfile1-dev
    librsvg2-dev
    libqalculate-dev
    libxml2-dev
    libmd4c-dev
    libtomlplusplus-dev
    libical-dev
    nlohmann-json3-dev
    libstb-dev
    libjemalloc-dev
    libgmp-dev
    libmpfr-dev
    libgtk4-layer-shell-dev
)

NIRI_PACKAGES=(
    libdrm-dev
    libpixman-1-dev
    libxkbcommon-dev
    libinput-dev
    libseat-dev
    libudev-dev
    libgbm-dev
    libdisplay-info-dev
    libdisplay-info3
    libliftoff-dev
    libliftoff0
    hwdata
    pnp.ids
    seatd
)

ALL_PACKAGES=("${COMMON_PACKAGES[@]}" "${NIRI_PACKAGES[@]}")

mkdir -p "$PKG_CACHE"
rm -rf "$DEV_ROOT"
mkdir -p "$DEV_ROOT"/{gmp,mpfr,noctalia,sdbus,librsvg,pipewire,niri,gtk4-layer-shell}

(
    cd "$PKG_CACHE"
    apt-get download "${ALL_PACKAGES[@]}"
)

extract_packages() {
    local destination="$1"
    shift
    local package
    for package in "$@"; do
        shopt -s nullglob
        local files=("$PKG_CACHE"/"$package"_*.deb)
        shopt -u nullglob
        [ "${#files[@]}" -gt 0 ] || {
            echo "downloaded package not found in cache: $package" >&2
            exit 1
        }
        for file in "${files[@]}"; do
            dpkg-deb -x "$file" "$destination"
        done
    done
}

extract_packages "$DEV_ROOT/noctalia" "${COMMON_PACKAGES[@]}"
extract_packages "$DEV_ROOT/gmp" libgmp-dev
extract_packages "$DEV_ROOT/mpfr" libmpfr-dev
extract_packages "$DEV_ROOT/sdbus" libsdbus-c++-dev
extract_packages "$DEV_ROOT/librsvg" librsvg2-dev
extract_packages "$DEV_ROOT/pipewire" libpipewire-0.3-dev libspa-0.2-dev libwireplumber-0.5-dev
extract_packages "$DEV_ROOT/niri" "${NIRI_PACKAGES[@]}"
extract_packages "$DEV_ROOT/gtk4-layer-shell" libgtk4-layer-shell-dev

for required in \
    "$DEV_ROOT/gmp/usr/include/x86_64-linux-gnu/gmp.h" \
    "$DEV_ROOT/mpfr/usr/include/mpfr.h" \
    "$DEV_ROOT/noctalia/usr/include/libqalculate/Number.h" \
    "$DEV_ROOT/noctalia/usr/include/nlohmann/json.hpp" \
    "$DEV_ROOT/sdbus/usr/include/sdbus-c++/sdbus-c++.h" \
    "$DEV_ROOT/librsvg/usr/include/librsvg-2.0/librsvg/rsvg.h" \
    "$DEV_ROOT/pipewire/usr/lib/x86_64-linux-gnu/pkgconfig/libpipewire-0.3.pc" \
    "$DEV_ROOT/pipewire/usr/lib/x86_64-linux-gnu/pkgconfig/libspa-0.2.pc" \
    "$DEV_ROOT/gtk4-layer-shell/usr/include/gtk4-layer-shell/gtk4-layer-shell.h"; do
    [ -e "$required" ] || {
        echo "desktop dev sysroot is incomplete: $required" >&2
        exit 1
    }
done

touch "$DEV_ROOT/.prepared"
printf '%s\n' "Prepared desktop development roots under $DEV_ROOT"
