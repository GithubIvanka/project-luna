#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_DESKTOP_ROOT_OUT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"

NIRI_TAG="${LUNA_NIRI_TAG:-v26.04}"
NOCTALIA_TAG="${LUNA_NOCTALIA_TAG:-v5.0.0-beta.8}"
NOCTALIA_GREETER_REF="${LUNA_NOCTALIA_GREETER_REF:-main}"
GHOSTTY_TAG="${LUNA_GHOSTTY_TAG:-1.3.1}"
GHOSTTY_ZIG_VERSION="${LUNA_GHOSTTY_ZIG_VERSION:-0.15.2}"
FISH_VERSION="${LUNA_FISH_VERSION:-4.8.1}"
WAYLAND_VERSION="${LUNA_WAYLAND_VERSION:-1.26.0}"
WAYLAND_PROTOCOLS_VERSION="${LUNA_WAYLAND_PROTOCOLS_VERSION:-1.49}"

mkdir -p "$OUT" "$SRC"
rm -rf "$OUT"
mkdir -p "$OUT/usr/bin" "$OUT/usr/lib" "$OUT/usr/share" "$OUT/etc/profile.d" "$OUT/etc/luna" "$OUT/var/lib/noctalia-greeter"

fetch_git() {
    local url="$1" ref="$2" dir="$3"
    if [ ! -d "$dir/.git" ]; then
        git clone --filter=blob:none --no-tags "$url" "$dir"
    fi
    git -C "$dir" fetch --depth 1 origin "$ref"
    git -C "$dir" checkout --force FETCH_HEAD
}

fetch_git https://github.com/niri-wm/niri.git "$NIRI_TAG" "$SRC/niri"
fetch_git https://github.com/noctalia-dev/noctalia.git "$NOCTALIA_TAG" "$SRC/noctalia"
fetch_git https://github.com/noctalia-dev/noctalia-greeter.git "$NOCTALIA_GREETER_REF" "$SRC/noctalia-greeter"
fetch_git https://github.com/ghostty-org/ghostty.git "$GHOSTTY_TAG" "$SRC/ghostty"
fetch_git https://gitlab.freedesktop.org/wayland/wayland.git "$WAYLAND_VERSION" "$SRC/wayland"
fetch_git https://gitlab.freedesktop.org/wayland/wayland-protocols.git "$WAYLAND_PROTOCOLS_VERSION" "$SRC/wayland-protocols"

if [ ! -d "$SRC/fish-$FISH_VERSION" ]; then
    curl -fsSL "https://github.com/fish-shell/fish-shell/releases/download/$FISH_VERSION/fish-$FISH_VERSION-linux-x86_64.tar.xz" -o "$SRC/fish-linux.tar.xz"
    tar -xf "$SRC/fish-linux.tar.xz" -C "$SRC"
fi

if ! command -v zig >/dev/null 2>&1 || ! zig version | grep -qx "$GHOSTTY_ZIG_VERSION"; then
    curl -fsSL "https://ziglang.org/download/$GHOSTTY_ZIG_VERSION/zig-linux-x86_64-$GHOSTTY_ZIG_VERSION.tar.xz" -o "$SRC/zig.tar.xz"
    tar -xf "$SRC/zig.tar.xz" -C "$SRC"
    export PATH="$SRC/zig-linux-x86_64-$GHOSTTY_ZIG_VERSION:$PATH"
fi

command -v cargo >/dev/null
command -v meson >/dev/null
command -v ninja >/dev/null
command -v zig >/dev/null
command -v cmake >/dev/null
command -v pkg-config >/dev/null
command -v ldd >/dev/null

# Build current Wayland libraries/protocol data first so every later component
# resolves against the Luna-pinned Wayland stack rather than the CI host copy.
(
    cd "$SRC/wayland"
    meson setup build-luna --buildtype=release --prefix=/usr -Dtests=false
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)
(
    cd "$SRC/wayland-protocols"
    meson setup build-luna --buildtype=release --prefix=/usr
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)
export PKG_CONFIG_SYSROOT_DIR="$OUT"
export PKG_CONFIG_PATH="$OUT/usr/lib/x86_64-linux-gnu/pkgconfig:$OUT/usr/lib/pkgconfig:$OUT/usr/share/pkgconfig"

# niri: compositor and the session launcher are both shipped, but the generic
# upstream session manager paths are replaced later with the Luna session.
(
    cd "$SRC/niri"
    cargo build --release
)
install -Dm0755 "$SRC/niri/target/release/niri" "$OUT/usr/bin/niri"
install -Dm0755 "$SRC/niri/resources/niri-session" "$OUT/usr/share/niri/upstream-niri-session"
install -Dm0644 "$SRC/niri/resources/niri.desktop" "$OUT/usr/share/wayland-sessions/niri.desktop"
install -Dm0644 "$SRC/niri/resources/niri-portals.conf" "$OUT/usr/share/xdg-desktop-portal/niri-portals.conf"

# Noctalia v5: native Wayland/OpenGL ES shell. Keep all shipped assets.
(
    cd "$SRC/noctalia"
    meson setup build-luna --buildtype=release --prefix=/usr \
        -Dnative_optimizations=false -Djemalloc=disabled -Dtests=disabled
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)

# Noctalia v5 greeter code is the GUI login surface. Luna will own the process
# lifecycle; the greeter itself is rebuilt from current upstream sources into a
# Luna-specific installation prefix and launched by luna-login.
(
    cd "$SRC/noctalia-greeter"
    meson setup build-luna --buildtype=release --prefix=/usr \
        -Db_lto=true
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)

# Ghostty 1.3.x requires Zig 0.15.2. Use its native Linux/GTK runtime so it
# integrates cleanly with niri and remains a normal Wayland application.
(
    cd "$SRC/ghostty"
    zig build -Doptimize=ReleaseFast -Dapp-runtime=gtk -fno-sys=gtk4-layer-shell -p "$OUT/usr"
)

# fish is the interactive shell. The current x86_64 release tarball avoids
# rebuilding the language runtime just to populate the immutable desktop image.
if [ -d "$SRC/fish-$FISH_VERSION" ]; then
    cp -a "$SRC/fish-$FISH_VERSION"/. "$OUT/"
else
    echo "fish release payload was not unpacked as expected" >&2
    exit 1
fi

# Compatibility shells: bash remains present and /bin/sh is POSIX-compatible.
install -Dm0755 "$(command -v bash)" "$OUT/usr/bin/bash"
ln -sfn /usr/bin/bash "$OUT/usr/bin/sh"
ln -sfn /usr/bin/fish "$OUT/usr/bin/luna-shell"

# Luna does not use niri's systemd/dinit session manager path. The graphical
# session is deliberately direct: luna-system-runtime owns niri as the session
# process and Noctalia is an niri startup service.
install -Dm0755 /dev/null "$OUT/usr/bin/niri-session"
cat > "$OUT/usr/bin/niri-session" <<'EOF'
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

exec /usr/bin/niri --session
EOF
chmod 0755 "$OUT/usr/bin/niri-session"

# Default Luna niri configuration: Noctalia is the shell; Ghostty+fish is the
# terminal; no Waybar/Quickshell/legacy shell is pulled into the image.
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

[versions]
niri = "$NIRI_TAG"
noctalia = "$NOCTALIA_TAG"
noctalia_greeter = "$NOCTALIA_GREETER_REF"
ghostty = "$GHOSTTY_TAG"
fish = "$FISH_VERSION"
wayland = "$WAYLAND_VERSION"
wayland_protocols = "$WAYLAND_PROTOCOLS_VERSION"
EOF

# Collect the complete glibc/graphics/Wayland dependency closure for all
# graphical ELF payloads. Luna-native Rust services remain separate and static.
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
            done < <(ldd "$elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib|usr\/lib)/ {print $1}')
        done < <(find "$root" -type f -perm -0100 -print0)
        [ "$changed" -eq 0 ] && break
    done
}

bundle_elf_deps "$OUT"

# The Noctalia greeter assets are needed even though its default runtime state
# is otherwise ephemeral. Keep the state directory writable on DATA later.
[ -d "$OUT/usr/share/noctalia/assets" ] || { echo "Noctalia assets missing" >&2; exit 1; }
[ -x "$OUT/usr/bin/noctalia" ] || { echo "Noctalia binary missing" >&2; exit 1; }
[ -x "$OUT/usr/bin/noctalia-greeter" ] || { echo "Noctalia greeter missing" >&2; exit 1; }
[ -x "$OUT/usr/bin/noctalia-greeter-compositor" ] || { echo "Noctalia greeter compositor missing" >&2; exit 1; }
[ -x "$OUT/usr/bin/ghostty" ] || { echo "Ghostty binary missing" >&2; exit 1; }
[ -x "$OUT/usr/bin/fish" ] || { echo "fish binary missing" >&2; exit 1; }

# luna-login is produced by the Luna workspace itself. This builder only
# validates the final integration point so the immutable SYSTEM image cannot be
# generated with a fake shell wrapper in place of the login UI.
if [ -x "$REPO_ROOT/target/release/luna-login" ]; then
    install -Dm0755 "$REPO_ROOT/target/release/luna-login" "$OUT/usr/bin/luna-login"
else
    echo "luna-login was not built; build the native Luna graphical login component first" >&2
    exit 1
fi

cat > "$OUT/etc/luna/desktop-ready" <<EOF
niri=$NIRI_TAG
noctalia=$NOCTALIA_TAG
noctalia_greeter=$NOCTALIA_GREETER_REF
ghostty=$GHOSTTY_TAG
fish=$FISH_VERSION
wayland=$WAYLAND_VERSION
wayland_protocols=$WAYLAND_PROTOCOLS_VERSION
EOF

printf '%s\n' "Built Project Luna desktop payload: $OUT"
