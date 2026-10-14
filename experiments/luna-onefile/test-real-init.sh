#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
IMAGE="${LUNA_ONEFILE_REAL_IMAGE:-$ROOT/out/luna-real-init.efi}"
INIT="${LUNA_ONEFILE_REAL_INIT:-$ROOT/out/luna-real-init.init}"
OUT="${LUNA_ONEFILE_REAL_TEST_OUT:-$ROOT/out/luna-real-init-test}"
SYSROOT="${LUNA_ONEFILE_REAL_SYSTEM_ROOT:-/tmp/luna-real-init-system-root-$$}"
RECOVERY_TEST="${LUNA_RECOVERY_TEST:-0}"
DATA_AMBIGUOUS_TEST="${LUNA_DATA_AMBIGUOUS_TEST:-0}"
DATA_CONFIG_TEST="${LUNA_DATA_CONFIG_TEST:-0}"
DATA_EXTERNAL_TEST="${LUNA_DATA_EXTERNAL_TEST:-0}"
DATA_CONFIG_STALE_TEST="${LUNA_DATA_CONFIG_STALE_TEST:-0}"
REAL_RUNTIME_TEST="${LUNA_REAL_SYSTEM_RUNTIME:-0}"
MANAGER_GATE_TEST="${LUNA_MANAGER_GATE_TEST:-0}"
MANAGER_GATE_FAILURE="${LUNA_MANAGER_GATE_FAILURE:-}"
MANAGER_GATE_INTERACTIVE_TEST="${LUNA_MANAGER_GATE_INTERACTIVE_TEST:-0}"
NIRI_SESSION_TEST="${LUNA_NIRI_SESSION_TEST:-0}"
NOCTALIA_SESSION_TEST="${LUNA_NOCTALIA_SESSION_TEST:-0}"
EXTRA_DATA_ROOT="${LUNA_ONEFILE_EXTRA_DATA_ROOT:-/tmp/luna-niri-provider-$$}"
RECOVERY_ROOT="${LUNA_ONEFILE_REAL_RECOVERY_ROOT:-/tmp/luna-recovery-data-root-$$}"

[ -f "$IMAGE" ] || { echo "missing real-init kernel: $IMAGE" >&2; exit 1; }
[ -f "$INIT" ] || { echo "missing real luna-init: $INIT" >&2; exit 1; }
if [ "$REAL_RUNTIME_TEST" = "1" ] && [ "$RECOVERY_TEST" != "1" ] && [ "$MANAGER_GATE_TEST" != "1" ]; then
  echo "real system runtime test requires Recovery mode unless the manager-gate test is enabled" >&2
  exit 1
fi
if [ "$MANAGER_GATE_TEST" = "1" ]; then
  case "$MANAGER_GATE_FAILURE" in
    ""|missing-volume-manager|no-available-volumes) ;;
    *) echo "unknown manager-gate failure mode: $MANAGER_GATE_FAILURE" >&2; exit 2 ;;
  esac
fi
if [ "$MANAGER_GATE_INTERACTIVE_TEST" = "1" ] && [ "$MANAGER_GATE_TEST" != "1" ]; then
  echo "interactive login test requires the manager-gate fixture" >&2
  exit 2
fi
if [ "$NOCTALIA_SESSION_TEST" = "1" ] && [ "$NIRI_SESSION_TEST" != "1" ]; then
  echo "Noctalia session test requires the Niri session fixture" >&2
  exit 2
fi
command -v rustc >/dev/null
rustup target list --installed | grep -qx x86_64-unknown-linux-musl

rm -rf "$SYSROOT"
mkdir -p "$SYSROOT/apps/luna-system-runtime" "$SYSROOT/config" "$SYSROOT/libs"
mkdir -p "$SYSROOT/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}

if [ "$REAL_RUNTIME_TEST" = "1" ]; then
  RUNTIME="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/luna-system-runtime"
  [ -x "$RUNTIME" ] || { echo "missing real luna-system-runtime: $RUNTIME" >&2; exit 1; }
  cp "$RUNTIME" "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"
  chmod 0755 "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"

  if [ "$NIRI_SESSION_TEST" = "1" ]; then
    if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
      python3 "$ROOT/stage-niri-test-provider.py" "$EXTRA_DATA_ROOT" --with-noctalia
    else
      python3 "$ROOT/stage-niri-test-provider.py" "$EXTRA_DATA_ROOT"
    fi
  fi

  if [ "$MANAGER_GATE_TEST" = "1" ]; then
    for manager in luna-device-manager luna-volume-manager; do
      if [ "$MANAGER_GATE_FAILURE" = "missing-volume-manager" ] && [ "$manager" = "luna-volume-manager" ]; then
        continue
      fi
      manager_dir="$SYSROOT/apps/$manager"
      mkdir -p "$manager_dir"
      if [ "$MANAGER_GATE_FAILURE" = "no-available-volumes" ] && [ "$manager" = "luna-volume-manager" ]; then
        # Deterministic faulty provider for the runtime's status-validation path.
        cat > "$SYSROOT/volume-manager-failure.rs" <<'EOF_VOLUME_MANAGER_FAILURE'
use std::{fs, thread, time::Duration};

fn main() -> ! {
    fs::create_dir_all("/run/luna").expect("runtime directory");
    fs::write(
        "/run/luna/volume-manager.status",
        "volume_count=1\navailable_count=0\nauto_attach=true\n",
    )
    .expect("write volume status");
    fs::write("/run/luna/volume-manager.ready", "ready\n")
        .expect("write ready marker");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
EOF_VOLUME_MANAGER_FAILURE
        rustc --target x86_64-unknown-linux-musl -O "$SYSROOT/volume-manager-failure.rs" -o "$manager_dir/luna-volume-manager"
        rm "$SYSROOT/volume-manager-failure.rs"
      else
        source="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/$manager"
        [ -x "$source" ] || { echo "missing required manager binary: $source" >&2; exit 1; }
        install -m0755 "$source" "$manager_dir/$manager"
      fi
    done
  fi
else
  cat > "$SYSROOT/runtime.rs" <<'EOF_RUNTIME'
use std::{path::Path, thread, time::Duration};

fn require_file(path: &str) {
    if !Path::new(path).is_file() {
        eprintln!("LUNA-LOGICAL-ROOT: missing file {path}");
        std::process::exit(20);
    }
}

fn require_dir(path: &str) {
    if !Path::new(path).is_dir() {
        eprintln!("LUNA-LOGICAL-ROOT: missing directory {path}");
        std::process::exit(21);
    }
}

fn require_symlink(path: &str) {
    if !Path::new(path).is_symlink() {
        eprintln!("LUNA-LOGICAL-ROOT: missing symlink {path}");
        std::process::exit(22);
    }
}

fn main() -> ! {
    for resource_class in ["fonts", "icons", "themes", "cursors", "sounds", "locales", "translations"] {
        require_file(&format!("/resources/{resource_class}/system-resource-marker"));
        require_file(&format!("/usr/share/{resource_class}/data-resource-marker"));
    }
    require_file("/config/system-config-marker");
    require_file("/etc/data-config-marker");
    require_file("/data/system/config/data-config-marker");
    require_file("/lib/luna/system-lib-marker");
    require_file("/usr/lib/data-lib-marker");
    require_file("/usr/local/lib/data-lib-marker");
    require_file("/usr/bin/data-probe");
    require_symlink("/lib/ld-musl-x86_64.so.1");

    let user = std::env::var("LUNA_SESSION_USER").unwrap_or_else(|_| "luna".to_owned());
    require_symlink(&format!("/home/{user}"));
    require_dir(&format!("/data/users/{user}/home"));

    if Path::new("/usr/bin/recovery-tools").is_file() {
        println!("LUNA-RECOVERY-DATA: READY");
    }
    println!("LUNA-LOGICAL-ROOT-CONTRACT: READY");
    println!("LUNA-SYSTEM-RUNTIME: READY");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
EOF_RUNTIME
rustc --target x86_64-unknown-linux-musl -O "$SYSROOT/runtime.rs" -o "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"
rm "$SYSROOT/runtime.rs"
chmod 0755 "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"
fi

cat > "$SYSROOT/config/passwd" <<'EOF_PASSWD'
root:x:0:0:root:/root:/bin/sh
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
recovery:x:1001:1001:Luna Recovery:/home/recovery:/bin/sh
EOF_PASSWD
if [ "$MANAGER_GATE_INTERACTIVE_TEST" = "1" ]; then
  # Recovery identities are not candidates for a normal graphical login.
  sed -i 's|^recovery:x:1001:1001:Luna Recovery:/home/recovery:/bin/sh$|recovery:x:1001:1001:Luna Recovery:/home/recovery:/sbin/nologin|' "$SYSROOT/config/passwd"
fi
cat > "$SYSROOT/config/group" <<'EOF_GROUP'
root:x:0:
shadow:x:42:
luna:x:1000:
recovery:x:1001:
seat:x:91:luna
video:x:27:luna
input:x:97:luna
EOF_GROUP
cat > "$SYSROOT/config/shadow" <<'EOF_SHADOW'
root:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
recovery:!*:0:0:99999:7:::
EOF_SHADOW
if [ "$MANAGER_GATE_INTERACTIVE_TEST" = "1" ]; then
  command -v openssl >/dev/null || { echo "interactive login test requires openssl" >&2; exit 1; }
  test_password_hash="$(openssl passwd -6 -salt lunatest 'test123')"
  python3 - "$SYSROOT/config/shadow" "$test_password_hash" <<'PY_SHADOW'
from pathlib import Path
import sys

path = Path(sys.argv[1])
password_hash = sys.argv[2]
lines = path.read_text().splitlines()
found = False
for index, line in enumerate(lines):
    fields = line.split(":")
    if fields[0] == "luna":
        fields[1] = password_hash
        lines[index] = ":".join(fields)
        found = True
        break
if not found:
    raise SystemExit("test account missing from shadow fixture")
path.write_text("\n".join(lines) + "\n")
PY_SHADOW
fi
printf '%s\n' "passwd: files" "group: files" "shadow: files" "gshadow: files" "hosts: files" "services: files" "networks: files" "protocols: files" > "$SYSROOT/config/nsswitch.conf"
mkdir -p "$SYSROOT/config/luna"
if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
  cat > "$SYSROOT/config/luna/niri-config.kdl" <<'EOF_NIRI_NOCTALIA'
input {
    keyboard {
        xkb {
            layout "us"
        }
    }
}
layout {}
hotkey-overlay {
    skip-at-startup
}
spawn-at-startup "/usr/bin/luna-noctalia-test"
EOF_NIRI_NOCTALIA
  : > "$SYSROOT/config/luna/noctalia-session-test"
else
  printf '%s\n' '// minimal Luna native-runtime test configuration' > "$SYSROOT/config/luna/niri-config.kdl"
fi
chmod 0644 "$SYSROOT/config/passwd" "$SYSROOT/config/group" "$SYSROOT/config/nsswitch.conf" "$SYSROOT/config/luna/niri-config.kdl"
chmod 0600 "$SYSROOT/config/shadow"

if [ "$REAL_RUNTIME_TEST" = "1" ]; then
  mkdir -p "$SYSROOT/apps/luna-graphical-session"
  if [ "$NIRI_SESSION_TEST" = "1" ]; then
    cat > "$SYSROOT/apps/luna-graphical-session/luna-graphical-session" <<'EOF_NIRI_SESSION'
#!/bin/sh
set -eu
export PATH=/usr/bin:/bin
export LIBSEAT_BACKEND=seatd
export XKB_CONFIG_ROOT=/usr/share/X11/xkb
export LD_LIBRARY_PATH=/usr/lib
export LIBGL_DRIVERS_PATH=/usr/lib/dri
export GBM_BACKENDS_PATH=/usr/lib/gbm
# Keep the compositor smoke test bounded on constrained test hosts.
export RAYON_NUM_THREADS=1
export FONTCONFIG_FILE=/etc/fonts/fonts.conf
export FONTCONFIG_PATH=/etc/fonts
export XDG_CACHE_HOME="$HOME/.cache"
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri
export XDG_SESSION_DESKTOP=niri
: "${XDG_RUNTIME_DIR:?Luna did not provide XDG_RUNTIME_DIR}"
/usr/bin/busybox mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CACHE_HOME/fontconfig"
/usr/bin/busybox chmod 700 "$XDG_RUNTIME_DIR"
echo "LUNA-NIRI: LAUNCHING /usr/bin/niri --session" >&2
/usr/bin/niri --session &
niri_pid=$!
noctalia_log_pid=""
if [ -f /etc/luna/noctalia-session-test ]; then
  /usr/bin/busybox touch "$XDG_RUNTIME_DIR/luna-noctalia.log"
  /usr/bin/busybox tail -f "$XDG_RUNTIME_DIR/luna-noctalia.log" >&2 &
  noctalia_log_pid=$!
  (
    count=0
    while [ "$count" -lt 80 ]; do
      for process in /proc/[0-9]*; do
        [ -r "$process/comm" ] || continue
        process_name=""
        IFS= read -r process_name < "$process/comm" || continue
        if [ "$process_name" = "noctalia" ]; then
          noctalia_process="$process"
          /usr/bin/busybox sleep 8
          if [ -r "$noctalia_process/comm" ]; then
            IFS= read -r process_name < "$noctalia_process/comm" || process_name=""
          fi
          if [ "$process_name" = "noctalia" ]; then
            echo "LUNA-NOCTALIA: PROCESS_ALIVE" >&2
            while [ -r "$noctalia_process/comm" ]; do
              IFS= read -r process_name < "$noctalia_process/comm" || process_name=""
              [ "$process_name" = "noctalia" ] || break
              /usr/bin/busybox sleep 5
            done
            echo "LUNA-NOCTALIA: PROCESS_EXITED" >&2
            exit 0
          fi
        fi
      done
      /usr/bin/busybox sleep 0.25
      count=$((count + 1))
    done
    echo "LUNA-NOCTALIA: PROCESS_NOT_FOUND" >&2
  ) &
fi
(
  count=0
  while [ "$count" -lt 80 ]; do
    for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
      if [ -S "$socket" ]; then
        echo "LUNA-NIRI: WAYLAND_READY $socket" >&2
        exit 0
      fi
    done
    /usr/bin/busybox sleep 0.25
    count=$((count + 1))
  done
  echo "LUNA-NIRI: WAYLAND_NOT_READY" >&2
) &
watchdog_pid=$!
set +e
wait "$niri_pid"
niri_status=$?
set -e
kill "$watchdog_pid" 2>/dev/null || true
wait "$watchdog_pid" 2>/dev/null || true
if [ -n "$noctalia_log_pid" ]; then
  kill "$noctalia_log_pid" 2>/dev/null || true
  wait "$noctalia_log_pid" 2>/dev/null || true
fi
exit "$niri_status"
EOF_NIRI_SESSION
  else
    cat > "$SYSROOT/session.rs" <<'EOF_SESSION'
use std::{path::Path, thread, time::Duration};

fn require(path: &str) {
    if !Path::new(path).exists() {
        eprintln!("LUNA-NATIVE-SESSION: missing {path}");
        std::process::exit(30);
    }
}

fn main() -> ! {
    for path in [
        "/etc/luna/niri-config.kdl",
        "/sbin/luna-system-runtime",
        "/data/system/state/luna-state.redb",
        "/etc/data-config-marker",
        "/usr/lib/data-lib-marker",
        "/usr/share/fonts/data-resource-marker",
        "/usr/bin/data-probe",
    ] {
        require(path);
    }
    println!("LUNA-NATIVE-SESSION: READY");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
EOF_SESSION
    rustc --target x86_64-unknown-linux-musl -O "$SYSROOT/session.rs" -o "$SYSROOT/apps/luna-graphical-session/luna-graphical-session"
    rm "$SYSROOT/session.rs"
  fi
  chmod 0755 "$SYSROOT/apps/luna-graphical-session/luna-graphical-session"
fi

if [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  rm -rf "$RECOVERY_ROOT"
  mkdir -p "$RECOVERY_ROOT/system"/{apps,libs,config,resources}
  mkdir -p "$RECOVERY_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
  mkdir -p "$RECOVERY_ROOT/system/apps/recovery-tools" "$RECOVERY_ROOT/system/apps/data-probe"
  : > "$RECOVERY_ROOT/system/libs/data-lib-marker"
  : > "$RECOVERY_ROOT/system/config/data-config-marker"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$RECOVERY_ROOT/system/apps/recovery-tools/recovery-tools"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$RECOVERY_ROOT/system/apps/data-probe/data-probe"
  chmod 0755 "$RECOVERY_ROOT/system/apps/recovery-tools/recovery-tools" "$RECOVERY_ROOT/system/apps/data-probe/data-probe"
  for resource_class in fonts icons themes cursors sounds locales translations; do
    : > "$RECOVERY_ROOT/system/resources/$resource_class/data-resource-marker"
  done
  trap 'rm -rf "$SYSROOT" "$RECOVERY_ROOT"' EXIT
elif [ "$NIRI_SESSION_TEST" = "1" ]; then
  trap 'rm -rf "$SYSROOT" "$EXTRA_DATA_ROOT"' EXIT
else
  trap 'rm -rf "$SYSROOT"' EXIT
fi

export LUNA_ONEFILE_IMAGE="$IMAGE"
export LUNA_ONEFILE_INIT="$INIT"
export LUNA_ONEFILE_SYSTEM_ROOT="$SYSROOT"
export LUNA_ONEFILE_DISK_TEST_OUT="$OUT"
if [ "$NIRI_SESSION_TEST" = "1" ]; then
  export LUNA_NIRI_SESSION_TEST=1
  export LUNA_ONEFILE_EXTRA_DATA_ROOT="$EXTRA_DATA_ROOT"
fi
if [ "$NOCTALIA_SESSION_TEST" = "1" ]; then
  export LUNA_NOCTALIA_SESSION_TEST=1
fi
if [ "$RECOVERY_TEST" = "1" ] || [ "$DATA_AMBIGUOUS_TEST" = "1" ] || [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  export LUNA_ONEFILE_RECOVERY_ROOT="$RECOVERY_ROOT"
fi
if [ "$DATA_AMBIGUOUS_TEST" = "1" ]; then
  export LUNA_DATA_AMBIGUOUS_TEST=1
fi
if [ "$DATA_CONFIG_TEST" = "1" ]; then
  export LUNA_DATA_CONFIG_TEST=1
fi
if [ "$DATA_EXTERNAL_TEST" = "1" ]; then
  export LUNA_DATA_EXTERNAL_TEST=1
fi
if [ "$DATA_CONFIG_STALE_TEST" = "1" ]; then
  export LUNA_DATA_CONFIG_STALE_TEST=1
fi
if [ "$REAL_RUNTIME_TEST" = "1" ]; then
  export LUNA_REAL_SYSTEM_RUNTIME=1
  export LUNA_NO_RESPAWN=1
fi
export LUNA_DIRECT_INIT=1
export LUNA_REAL_INIT=1

cd "$ROOT"
./test-luna-sys.sh
