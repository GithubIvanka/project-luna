#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_DESKTOP_ROOT_OUT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"

NIRI_TAG="${LUNA_NIRI_TAG:-v26.04}"
NOCTALIA_TAG="${LUNA_NOCTALIA_TAG:-v5.0.0-beta.8}"
GHOSTTY_TAG="${LUNA_GHOSTTY_TAG:-1.3.1}"
FISH_VERSION="${LUNA_FISH_VERSION:-4.1.2}"

mkdir -p "$OUT" "$SRC"
rm -rf "$OUT"
mkdir -p "$OUT/usr/bin" "$OUT/usr/lib" "$OUT/usr/share/wayland-sessions" "$OUT/usr/share/xdg-desktop-portal" "$OUT/usr/share/noctalia" "$OUT/etc/profile.d" "$OUT/etc/luna"

fetch_git() {
    local url="$1" ref="$2" dir="$3"
    if [ ! -d "$dir/.git" ]; then
        git clone --filter=blob:none --no-tags "$url" "$dir"
    fi
    git -C "$dir" fetch --depth 1 origin "$ref"
    git -C "$dir" checkout --force FETCH_HEAD
}

fetch_git https://github.com/YaLTeR/niri.git "$NIRI_TAG" "$SRC/niri"
fetch_git https://github.com/noctalia-dev/noctalia.git "$NOCTALIA_TAG" "$SRC/noctalia"
fetch_git https://github.com/ghostty-org/ghostty.git "$GHOSTTY_TAG" "$SRC/ghostty"

if [ ! -d "$SRC/fish-$FISH_VERSION" ]; then
    curl -fsSL "https://github.com/fish-shell/fish-shell/releases/download/$FISH_VERSION/fish-$FISH_VERSION.tar.xz" -o "$SRC/fish.tar.xz"
    tar -xf "$SRC/fish.tar.xz" -C "$SRC"
fi

if command -v apt-get >/dev/null 2>&1; then
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
        build-essential clang pkg-config meson ninja-build cmake curl git \
        libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev \
        libinput-dev libdbus-1-dev libsystemd-dev libseat-dev libpipewire-0.3-dev \
        libpango1.0-dev libdisplay-info-dev libpam0g-dev \
        libdrm-dev libxkbcommon-x11-dev libwayland-egl1-mesa \
        libssl-dev libpcre2-dev libedit-dev libncurses-dev
fi

if ! command -v zig >/dev/null 2>&1; then
    ZIG_VERSION="0.13.0"
    curl -fsSL "https://ziglang.org/download/$ZIG_VERSION/zig-linux-x86_64-$ZIG_VERSION.tar.xz" -o "$SRC/zig.tar.xz"
    tar -xf "$SRC/zig.tar.xz" -C "$SRC"
    export PATH="$SRC/zig-linux-x86_64-$ZIG_VERSION:$PATH"
fi

command -v cargo >/dev/null
command -v meson >/dev/null
command -v ninja >/dev/null
command -v zig >/dev/null

# niri: build the standalone compositor/session binaries from the pinned source.
(
    cd "$SRC/niri"
    cargo build --release
)
cp "$SRC/niri/target/release/niri" "$OUT/usr/bin/niri"
cp "$SRC/niri/target/release/niri-session" "$OUT/usr/bin/niri-session"
chmod 0755 "$OUT/usr/bin/niri" "$OUT/usr/bin/niri-session"

# Install niri's desktop session metadata when present in the source tree.
if [ -f "$SRC/niri/resources/niri.desktop" ]; then
    cp "$SRC/niri/resources/niri.desktop" "$OUT/usr/share/wayland-sessions/niri.desktop"
elif [ -f "$SRC/niri/resources/niri-session.desktop" ]; then
    cp "$SRC/niri/resources/niri-session.desktop" "$OUT/usr/share/wayland-sessions/niri.desktop"
fi

# Noctalia v5 is built natively with Meson/Ninja. Keep assets beside the binary.
(
    cd "$SRC/noctalia"
    meson setup build --buildtype=release --prefix=/usr
    meson compile -C build -j "$JOBS"
)
if [ -f "$SRC/noctalia/build/noctalia" ]; then
    cp "$SRC/noctalia/build/noctalia" "$OUT/usr/bin/noctalia"
elif [ -f "$SRC/noctalia/build/src/noctalia" ]; then
    cp "$SRC/noctalia/build/src/noctalia" "$OUT/usr/bin/noctalia"
else
    echo "Noctalia binary not found after Meson build" >&2
    exit 1
fi
chmod 0755 "$OUT/usr/bin/noctalia"
if [ -d "$SRC/noctalia/assets" ]; then
    cp -a "$SRC/noctalia/assets/." "$OUT/usr/share/noctalia/"
fi
if [ -f "$SRC/noctalia/noctalia.desktop" ]; then
    cp "$SRC/noctalia/noctalia.desktop" "$OUT/usr/share/noctalia/noctalia.desktop"
fi

# Ghostty: build the Linux application from the pinned release source.
(
    cd "$SRC/ghostty"
    zig build -Doptimize=ReleaseFast -Dapp-runtime=gtk
)
cp "$SRC/ghostty/zig-out/bin/ghostty" "$OUT/usr/bin/ghostty"
chmod 0755 "$OUT/usr/bin/ghostty"

# fish: build/install only the shell and its functions/completions.
(
    cd "$SRC/fish-$FISH_VERSION"
    cmake -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr -DWITH_GETTEXT=OFF
    cmake --build build -j "$JOBS"
    DESTDIR="$OUT" cmake --install build
)

# Bash and /bin/sh are compatibility shells; fish remains the interactive default.
if command -v bash >/dev/null 2>&1; then
    install -Dm0755 "$(command -v bash)" "$OUT/usr/bin/bash"
fi
if [ -x "$OUT/usr/bin/bash" ]; then
    ln -sf bash "$OUT/usr/bin/sh"
fi
if [ -x "$OUT/usr/bin/fish" ]; then
    ln -sf /usr/bin/fish "$OUT/usr/bin/luna-shell"
fi

cat > "$OUT/etc/profile" <<'EOF'
export PATH=/usr/local/bin:/usr/bin:/bin
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export XDG_SESSION_TYPE=wayland
export MOZ_ENABLE_WAYLAND=1
export QT_QPA_PLATFORM=wayland
export SDL_VIDEODRIVER=wayland
if [ -x /usr/bin/fish ]; then
    export SHELL=/usr/bin/fish
fi
EOF

cat > "$OUT/etc/luna/desktop.toml" <<EOF
[desktop]
compositor = "niri"
shell = "noctalia"
terminal = "ghostty"
interactive_shell = "fish"
compatibility_shell = "bash"
posix_shell = "sh"

[versions]
niri = "$NIRI_TAG"
noctalia = "$NOCTALIA_TAG"
ghostty = "$GHOSTTY_TAG"
fish = "$FISH_VERSION"
wayland = "system"
EOF

# Keep runtime discovery deterministic for the PC-image builder.
touch "$OUT/etc/luna/desktop-ready"

echo "Built Project Luna desktop payload: $OUT"
