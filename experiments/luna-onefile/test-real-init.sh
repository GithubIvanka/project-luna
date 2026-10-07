#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"
IMAGE="${LUNA_ONEFILE_REAL_IMAGE:-$ROOT/out/luna-real-init.efi}"
INIT="${LUNA_ONEFILE_REAL_INIT:-$ROOT/out/luna-real-init.init}"
OUT="${LUNA_ONEFILE_REAL_TEST_OUT:-$ROOT/out/luna-real-init-test}"
SYSROOT="${LUNA_ONEFILE_REAL_SYSTEM_ROOT:-/tmp/luna-real-init-system-root-$$}"
RECOVERY_TEST="${LUNA_RECOVERY_TEST:-0}"
RECOVERY_ROOT="${LUNA_ONEFILE_REAL_RECOVERY_ROOT:-/tmp/luna-recovery-data-root-$$}"

[ -f "$IMAGE" ] || { echo "missing real-init kernel: $IMAGE" >&2; exit 1; }
[ -f "$INIT" ] || { echo "missing real luna-init: $INIT" >&2; exit 1; }
command -v rustc >/dev/null
rustup target list --installed | grep -qx x86_64-unknown-linux-musl

rm -rf "$SYSROOT"
mkdir -p "$SYSROOT/apps/luna-system-runtime" "$SYSROOT/config" "$SYSROOT/libs"
mkdir -p "$SYSROOT/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}

cat > "$SYSROOT/runtime.rs" <<'EOF_RUNTIME'
use std::{thread, time::Duration};

fn main() -> ! {
    if std::path::Path::new("/usr/bin/recovery-tools").is_file() {
        println!("LUNA-RECOVERY-DATA: READY");
    }
    println!("LUNA-SYSTEM-RUNTIME: READY");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
EOF_RUNTIME
rustc --target x86_64-unknown-linux-musl -O "$SYSROOT/runtime.rs" -o "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"
rm "$SYSROOT/runtime.rs"
chmod 0755 "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"

cat > "$SYSROOT/config/passwd" <<'EOF_PASSWD'
root:x:0:0:root:/root:/bin/sh
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
EOF_PASSWD
cat > "$SYSROOT/config/group" <<'EOF_GROUP'
root:x:0:
shadow:x:42:
luna:x:1000:
EOF_GROUP
cat > "$SYSROOT/config/shadow" <<'EOF_SHADOW'
root:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
EOF_SHADOW
printf '%s\n' "passwd: files" "group: files" "shadow: files" "gshadow: files" "hosts: files" "services: files" "networks: files" "protocols: files" > "$SYSROOT/config/nsswitch.conf"
chmod 0644 "$SYSROOT/config/passwd" "$SYSROOT/config/group" "$SYSROOT/config/nsswitch.conf"
chmod 0600 "$SYSROOT/config/shadow"

if [ "$RECOVERY_TEST" = "1" ]; then
  rm -rf "$RECOVERY_ROOT"
  mkdir -p "$RECOVERY_ROOT/system"/{apps,libs,config,resources}
  mkdir -p "$RECOVERY_ROOT/system/resources"/{fonts,icons,themes,cursors,sounds,locales,translations}
  mkdir -p "$RECOVERY_ROOT/system/apps/recovery-tools"
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$RECOVERY_ROOT/system/apps/recovery-tools/recovery-tools"
  chmod 0755 "$RECOVERY_ROOT/system/apps/recovery-tools/recovery-tools"
  trap 'rm -rf "$SYSROOT" "$RECOVERY_ROOT"' EXIT
else
  trap 'rm -rf "$SYSROOT"' EXIT
fi

export LUNA_ONEFILE_IMAGE="$IMAGE"
export LUNA_ONEFILE_INIT="$INIT"
export LUNA_ONEFILE_SYSTEM_ROOT="$SYSROOT"
export LUNA_ONEFILE_DISK_TEST_OUT="$OUT"
if [ "$RECOVERY_TEST" = "1" ]; then
  export LUNA_ONEFILE_RECOVERY_ROOT="$RECOVERY_ROOT"
fi
export LUNA_DIRECT_INIT=1
export LUNA_REAL_INIT=1

cd "$ROOT"
./test-luna-sys.sh
