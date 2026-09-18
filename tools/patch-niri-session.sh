#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/desktop-root}"

install -Dm0755 /dev/null "$ROOT/usr/bin/niri-session"
cat > "$ROOT/usr/bin/niri-session" <<'EOF'
#!/bin/sh
set -eu

export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
export MOZ_ENABLE_WAYLAND=1
export QT_QPA_PLATFORM=wayland
export SDL_VIDEODRIVER=wayland
export XDG_DATA_DIRS="/usr/local/share:/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"

: "${HOME:=/home/luna}"
: "${USER:=luna}"
: "${LOGNAME:=$USER}"
: "${XDG_RUNTIME_DIR:=/run/user/$(id -u)}"
export HOME USER LOGNAME XDG_RUNTIME_DIR

mkdir -p "$HOME/.config/niri" "$XDG_RUNTIME_DIR"
chmod 0700 "$XDG_RUNTIME_DIR"
if [ ! -e "$HOME/.config/niri/config.kdl" ]; then
    cp /etc/luna/niri-config.kdl "$HOME/.config/niri/config.kdl"
fi

pids=""
cleanup() {
    if [ -n "$pids" ]; then
        kill $pids 2>/dev/null || true
    fi
    wait 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

# Audio belongs to the authenticated user session, not the system runtime.
/usr/bin/pipewire >/dev/null 2>&1 &
pids="$pids $!"
if [ -x /usr/bin/pipewire-pulse ]; then
    /usr/bin/pipewire-pulse >/dev/null 2>&1 &
    pids="$pids $!"
fi
/usr/bin/wireplumber >/dev/null 2>&1 &
pids="$pids $!"

exec /usr/bin/niri --session
EOF
chmod 0755 "$ROOT/usr/bin/niri-session"

printf '%s\n' '/usr/bin/niri-session' > "$ROOT/etc/luna/graphical-session"
