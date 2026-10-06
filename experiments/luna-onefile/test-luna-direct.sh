#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMAGE="${LUNA_ONEFILE_DIRECT_IMAGE:-${ROOT}/out/luna-onefile-direct.efi}"
CONFIG="${LUNA_ONEFILE_DIRECT_CONFIG:-${ROOT}/out/luna-onefile-direct.config}"
OUT="${LUNA_ONEFILE_DIRECT_TEST_OUT:-${ROOT}/out/luna-direct-test}"

[ -f "$IMAGE" ] || { echo "missing direct-init image: $IMAGE" >&2; exit 1; }
[ -f "$CONFIG" ] || { echo "missing direct-init config: $CONFIG" >&2; exit 1; }

if grep -q "^CONFIG_INITRAMFS_SOURCE=" "$CONFIG"; then
  echo "direct-init image must not use CONFIG_INITRAMFS_SOURCE" >&2
  exit 1
fi
grep -q "^CONFIG_RUST=y$" "$CONFIG"
grep -q "^CONFIG_RELOCATABLE=y$" "$CONFIG"

export LUNA_ONEFILE_IMAGE="$IMAGE"
export LUNA_DIRECT_INIT=1
export LUNA_ONEFILE_DISK_TEST_OUT="$OUT"
cd "$ROOT"
./test-luna-sys.sh

grep -Fq "luna_exec: Luna: preparing memory-resident luna-init" "$OUT/serial.log"
grep -Fq "luna_exec: Luna: executing memory-resident luna-init as PID 1" "$OUT/serial.log"
grep -Fq "LUNA-READY" "$OUT/serial.log"

if grep -Eq "Kernel panic|general protection|No working init|Failed to execute" "$OUT/serial.log"; then
  echo "direct-init acceptance failed; fatal boot error detected" >&2
  exit 1
fi

echo "Direct-init acceptance passed: bootloader -> external luna-init -> LUNA-READY"
