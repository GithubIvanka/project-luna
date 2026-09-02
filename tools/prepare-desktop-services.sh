#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/desktop-root}"
mkdir -p "$ROOT/usr/bin" "$ROOT/usr/sbin" "$ROOT/usr/lib" "$ROOT/etc/luna/services" "$ROOT/etc/pipewire" "$ROOT/etc/wireplumber" "$ROOT/etc/NetworkManager/system-connections" "$ROOT/etc/bluetooth"

copy_exec() {
  local src="$1" dst="$2"
  [ -x "$src" ] || { echo "required runtime missing: $src" >&2; exit 1; }
  install -Dm0755 "$src" "$ROOT/$dst"
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

# D-Bus is the control plane used by NetworkManager, BlueZ and WirePlumber.
copy_exec /usr/bin/dbus-daemon usr/bin/dbus-daemon
copy_exec /usr/bin/dbus-send usr/bin/dbus-send
copy_exec /usr/bin/dbus-run-session usr/bin/dbus-run-session
if [ -x /usr/bin/gdbus ]; then copy_exec /usr/bin/gdbus usr/bin/gdbus; fi

# USB/removable media integration for the future Luna device manager. UDisks2
# is kept as a backend helper while Luna owns the user-facing mount policy.
if [ -x /usr/libexec/udisks2/udisksd ]; then copy_exec /usr/libexec/udisks2/udisksd usr/libexec/udisks2/udisksd; fi
if [ -x /usr/bin/udisksctl ]; then copy_exec /usr/bin/udisksctl usr/bin/udisksctl; fi

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
user = "root"

[processes]
network_manager = "/usr/sbin/NetworkManager"
EOF
cat > "$ROOT/etc/luna/services/bluetooth.toml" <<'EOF'
[service]
name = "bluetooth"
implementation = "bluez"
user = "root"

[processes]
bluetoothd = "/usr/libexec/bluetooth/bluetoothd"
EOF
cat > "$ROOT/etc/luna/services/removable-media.toml" <<'EOF'
[service]
name = "removable-media"
implementation = "udisks2"
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
# Audio processes move into the authenticated user's session; network,
# Bluetooth and removable-media daemons stay system-owned.
include = [
  "/etc/luna/services/network.toml",
  "/etc/luna/services/bluetooth.toml",
  "/etc/luna/services/removable-media.toml",
]
EOF

echo "Prepared Luna audio/network/bluetooth/removable-media payload in $ROOT"
