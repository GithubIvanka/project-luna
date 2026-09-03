#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_DESKTOP_ROOT_OUT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"

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
    git -C "$dir" fetch --depth 1 origin "$ref"
    git -C "$dir" checkout --force FETCH_HEAD
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
if [ ! -d "$FISH_DIR" ]; then
    curl -fsSL "https://github.com/fish-shell/fish-shell/releases/download/$FISH_VERSION/fish-$FISH_VERSION-linux-x86_64.tar.xz" -o "$SRC/fish-linux.tar.xz"
    tar -xf "$SRC/fish-linux.tar.xz" -C "$SRC"
fi
if [ ! -d "$FISH_DIR" ]; then
    FISH_DIR="$(find "$SRC" -maxdepth 1 -mindepth 1 -type d -name "fish-$FISH_VERSION-*" -print -quit)"
fi
[ -n "$FISH_DIR" ] && [ -d "$FISH_DIR" ] || { echo "fish payload directory not found" >&2; exit 1; }

if ! command -v zig >/dev/null 2>&1 || ! zig version | grep -qx "$GHOSTTY_ZIG_VERSION"; then
    curl -fsSL "https://ziglang.org/download/$GHOSTTY_ZIG_VERSION/zig-linux-x86_64-$GHOSTTY_ZIG_VERSION.tar.xz" -o "$SRC/zig.tar.xz"
    tar -xf "$SRC/zig.tar.xz" -C "$SRC"
    export PATH="$SRC/zig-linux-x86_64-$GHOSTTY_ZIG_VERSION:$PATH"
fi

for tool in cargo meson ninja zig cmake pkg-config ldd python3; do
    command -v "$tool" >/dev/null || { echo "missing required build tool: $tool" >&2; exit 1; }
done

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
export PKG_CONFIG_PATH="$OUT/usr/lib/pkgconfig:$OUT/usr/share/pkgconfig:$OUT/usr/lib/x86_64-linux-gnu/pkgconfig"

(
    cd "$SRC/wlroots"
    meson setup build-luna --buildtype=release --prefix=/usr -Dxwayland=disabled -Dexamples=false
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)
(
    cd "$SRC/wireplumber"
    meson setup build-luna --buildtype=release --prefix=/usr
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)

python3 - "$SRC/noctalia-greeter/meson.build" <<'PY'
from pathlib import Path
p = Path(__import__('sys').argv[1])
s = p.read_text()
s = s.replace("    '-march=native', '-mtune=native',\n", "")
p.write_text(s)
PY

(
    cd "$SRC/niri"
    cargo build --release
)
install -Dm0755 "$SRC/niri/target/release/niri" "$OUT/usr/bin/niri"
install -Dm0755 "$SRC/niri/resources/niri-session" "$OUT/usr/share/niri/upstream-niri-session"
install -Dm0644 "$SRC/niri/resources/niri.desktop" "$OUT/usr/share/wayland-sessions/niri.desktop"
install -Dm0644 "$SRC/niri/resources/niri-portals.conf" "$OUT/usr/share/xdg-desktop-portal/niri-portals.conf"

(
    cd "$SRC/noctalia"
    meson setup build-luna --buildtype=release --prefix=/usr -Dnative_optimizations=false -Djemalloc=disabled -Dtests=disabled
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)

(
    cd "$SRC/noctalia-greeter"
    meson setup build-luna --buildtype=release --prefix=/usr -Db_lto=true
    meson compile -C build-luna -j "$JOBS"
    DESTDIR="$OUT" meson install -C build-luna
)

(
    cd "$SRC/greetd"
    cargo build --release
)
install -Dm0755 "$SRC/greetd/target/release/greetd" "$OUT/usr/bin/greetd"

(
    cd "$SRC/ghostty"
    zig build -Doptimize=ReleaseFast -Dapp-runtime=gtk -p "$OUT/usr"
)

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
if command -v dbus-run-session >/dev/null 2>&1; then
    exec dbus-run-session -- /usr/bin/niri --session
fi
exec /usr/bin/niri --session
EOF
chmod 0755 "$OUT/usr/bin/niri-session"

cat > "$OUT/usr/bin/luna-run-session" <<'EOF'
#!/bin/sh
set -eu
uid="$(id -u luna)"
gid="$(id -g luna)"
exec /usr/bin/setpriv --reuid="$uid" --regid="$gid" --init-groups -- /usr/bin/niri-session
EOF
chmod 0755 "$OUT/usr/bin/luna-run-session"

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
Exec=/usr/bin/luna-login --handoff
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

cargo build --release -p luna-login
install -Dm0755 "$REPO_ROOT/target/release/luna-login" "$OUT/usr/bin/luna-login"

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

PAM_UNIX="$(find /usr/lib /lib -path '*/security/pam_unix.so' -print -quit)"
[ -n "$PAM_UNIX" ] || { echo "pam_unix.so not found on CI host" >&2; exit 1; }
install -Dm0644 "$PAM_UNIX" "$OUT/${PAM_UNIX#/}"
while IFS= read -r dep; do
    case "$dep" in
        /lib/*|/lib64/*|/usr/lib/*)
            rel="${dep#/}"
            [ -e "$OUT/$rel" ] || { mkdir -p "$(dirname "$OUT/$rel")"; cp -a "$dep" "$OUT/$rel"; }
            ;;
    esac
done < <(ldd "$PAM_UNIX" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib|usr\/lib)/ {print $1}')

cat > "$OUT/etc/luna/desktop-ready" <<'EOF'
Project Luna graphical desktop payload
niri + Noctalia + Ghostty + fish + Luna Files
EOF
