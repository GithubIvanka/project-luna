#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/desktop-root}"
mkdir -p "$ROOT/usr/bin" "$ROOT/usr/sbin" "$ROOT/usr/lib" "$ROOT/etc/luna/services" "$ROOT/etc/pipewire" "$ROOT/etc/wireplumber" "$ROOT/etc/NetworkManager/system-connections" "$ROOT/etc/bluetooth" "$ROOT/etc/dbus-1/system.d" "$ROOT/usr/share/dbus-1"

copy_exec() {
  local src="$1" dst="$2"
  [ -x "$src" ] || { echo "required runtime missing: $src" >&2; exit 1; }
  install -Dm0755 "$src" "$ROOT/$dst"
}

copy_optional() {
  local src="$1" dst="$2"
  [ -e "$src" ] || return 0
  mkdir -p "$ROOT/$(dirname "$dst")"
  cp -a "$src" "$ROOT/$dst"
}

# Runtime services. These remain implementation details behind Luna's service
# lifecycle; users interact through Noctalia and Luna APIs, not service names.
copy_exec /usr/sbin/NetworkManager usr/sbin/NetworkManager
copy_exec /usr/bin/nmcli usr/bin/nmcli
copy_exec /usr/bin/bluetoothctl usr/bin/bluetoothctl
copy_exec /usr/libexec/bluetooth/bluetoothd usr/libexec/bluetooth/bluetoothd
copy_exec /usr/bin/pipewire usr/bin/pipewire
copy_exec /usr/bin/pipewire-pulse usr/bin/pipewire-pulse
copy_exec /usr/bin/wireplumber usr/bin/wireplumber
copy_exec /usr/bin/pw-cli usr/bin/pw-cli
copy_exec /usr/bin/pactl usr/bin/pactl
copy_exec /usr/sbin/rfkill usr/sbin/rfkill

# D-Bus is the system/session control plane used by NetworkManager, BlueZ,
# UDisks2 and the desktop audio stack.
copy_exec /usr/bin/dbus-daemon usr/bin/dbus-daemon
copy_exec /usr/bin/dbus-send usr/bin/dbus-send
copy_exec /usr/bin/dbus-run-session usr/bin/dbus-run-session
if [ -x /usr/bin/gdbus ]; then copy_exec /usr/bin/gdbus usr/bin/gdbus; fi

copy_optional /usr/share/dbus-1/system.conf /usr/share/dbus-1/system.conf
if [ -d /etc/dbus-1/system.d ]; then
  while IFS= read -r -d '' conf; do
    install -Dm0644 "$conf" "$ROOT/etc/dbus-1/system.d/$(basename "$conf")"
  done < <(find /etc/dbus-1/system.d -maxdepth 1 -type f -name '*.conf' -print0)
fi

# NetworkManager and BlueZ policy/configuration. Avoid copying runtime state or
# host-specific connection secrets into the immutable image.
copy_optional /etc/NetworkManager/NetworkManager.conf /etc/NetworkManager/NetworkManager.conf
copy_optional /etc/NetworkManager/conf.d /etc/NetworkManager/conf.d
copy_optional /etc/bluetooth/main.conf /etc/bluetooth/main.conf
copy_optional /etc/bluetooth/input.conf /etc/bluetooth/input.conf

# USB/removable media integration for the Luna device manager. UDisks2 remains
# the backend helper while Luna owns user-facing mount policy.
if [ -x /usr/libexec/udisks2/udisksd ]; then copy_exec /usr/libexec/udisks2/udisksd usr/libexec/udisks2/udisksd; fi
if [ -x /usr/bin/udisksctl ]; then copy_exec /usr/bin/udisksctl usr/bin/udisksctl; fi
if [ -d /usr/lib/udisks2 ]; then copy_optional /usr/lib/udisks2 /usr/lib/udisks2; fi

# Basic daemon configuration.
cat > "$ROOT/etc/pipewire/pipewire.conf" <<'EOF'
context.properties = {
    log.level = 0
}
EOF
cat > "$ROOT/etc/wireplumber/wireplumber.conf" <<'EOF'
wireplumber.profiles = { main = { } }
EOF
cat > "$ROOT/etc/luna/services/audio.toml" <<'EOF'
[service]
name = "audio"
implementation = "pipewire"
scope = "user-session"
user = "luna"

[processes]
pipewire = "/usr/bin/pipewire"
pipewire_pulse = "/usr/bin/pipewire-pulse"
wireplumber = "/usr/bin/wireplumber"
EOF
cat > "$ROOT/etc/luna/services/network.toml" <<'EOF'
[service]
name = "network"
implementation = "networkmanager"
scope = "system"
user = "root"

[processes]
network_manager = "/usr/sbin/NetworkManager"
EOF
cat > "$ROOT/etc/luna/services/bluetooth.toml" <<'EOF'
[service]
name = "bluetooth"
implementation = "bluez"
scope = "system"
user = "root"

[processes]
bluetoothd = "/usr/libexec/bluetooth/bluetoothd"
EOF
cat > "$ROOT/etc/luna/services/removable-media.toml" <<'EOF'
[service]
name = "removable-media"
implementation = "udisks2"
scope = "system"
user = "root"

[processes]
udisksd = "/usr/libexec/udisks2/udisksd"
EOF

# Bundle ELF dependencies for the newly added services into the immutable
# desktop root. This keeps the image independent from the CI host runtime libs.
bundle_elf_deps() {
  local root="$1" pass=0
  while [ "$pass" -lt 8 ]; do
    pass=$((pass + 1)); local changed=0
    while IFS= read -r -d '' elf; do
      file "$elf" | grep -q 'ELF' || continue
      while IFS= read -r dep; do
        case "$dep" in
          /lib/*|/lib64/*|/usr/lib/*)
            [ -e "$dep" ] || continue
            local rel="${dep#/}" dst="$root/$rel"
            if [ ! -e "$dst" ]; then mkdir -p "$(dirname "$dst")"; cp -a "$dep" "$dst"; changed=1; fi
            ;;
        esac
      done < <(ldd "$elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib|usr\/lib)/ {print $1}')
    done < <(find "$root" -type f -perm -0100 -print0)
    [ "$changed" -eq 0 ] && break
  done
}

command -v ldd >/dev/null
command -v file >/dev/null
bundle_elf_deps "$ROOT"

cat > "$ROOT/etc/luna/services.toml" <<'EOF'
# Luna host services. luna-system-runtime is the owner of these processes.
# Audio runs inside the authenticated user session; network, Bluetooth and
# removable-media daemons remain system-owned behind the system D-Bus bus.
include = [
  "/etc/luna/services/network.toml",
  "/etc/luna/services/bluetooth.toml",
  "/etc/luna/services/removable-media.toml",
]
EOF

# Runtime directories are created by the system runtime; these paths make the
# ownership explicit without baking machine-specific sockets into the image.
mkdir -p "$ROOT/run/dbus" "$ROOT/run/media/luna"

echo "Prepared Luna audio/network/bluetooth/removable-media payload in $ROOT"
