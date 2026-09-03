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

install -Dm0755 /dev/null "$ROOT/usr/bin/luna-run-session"
cat > "$ROOT/usr/bin/luna-run-session" <<'EOF'
#!/bin/sh
set -eu

user="${LUNA_SESSION_USER:-luna}"
uid="$(id -u "$user")"
gid="$(id -g "$user")"

runtime="/run/user/$uid"
mkdir -p "$runtime"
chown "$uid:$gid" "$runtime"
chmod 0700 "$runtime"

home="$(awk -F: -v wanted="$user" '$1 == wanted { print $6; exit }' /etc/passwd)"
[ -n "$home" ] || { echo "cannot resolve home directory for $user" >&2; exit 1; }

export HOME="$home"
export USER="$user"
export LOGNAME="$user"
export XDG_RUNTIME_DIR="$runtime"

exec /usr/bin/setpriv \
    --reuid="$uid" \
    --regid="$gid" \
    --init-groups \
    --env=HOME="$HOME" \
    --env=USER="$USER" \
    --env=LOGNAME="$LOGNAME" \
    --env=XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
    -- /usr/bin/niri-session
EOF
chmod 0755 "$ROOT/usr/bin/luna-run-session"

printf '%s\n' '/usr/bin/luna-run-session' > "$ROOT/etc/luna/graphical-session"
