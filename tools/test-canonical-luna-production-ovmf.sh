#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMG="$REPO_ROOT/dist/luna-0.1.0.img"
TEST_IMG="$REPO_ROOT/dist/.build/canonical-production-test.img"
VARS="$REPO_ROOT/dist/.build/canonical-production-test-vars.fd"
LOG="$REPO_ROOT/dist/logs/canonical-production-test-serial.log"
QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-sdl,gl=on}"

cp --reflink=auto "$IMG" "$TEST_IMG"
cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS"
rm -f "$LOG"

set +e
timeout 20s qemu-system-x86_64 -machine q35 -m 2G -smp 4 -accel kvm -cpu host   -vga none -device virtio-gpu-gl-pci   -nic user,model=virtio-net-pci   -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd   -drive if=pflash,format=raw,file="$VARS"   -drive format=raw,snapshot=on,file="$TEST_IMG"   -serial "file:$LOG" -display "$QEMU_DISPLAY" -no-reboot -no-shutdown
rc=$?
set -e

[ "$rc" -eq 124 ] || { echo "unexpected QEMU exit: $rc" >&2; exit "$rc"; }

grep -q "luna_boot: Luna: handoff v1 accepted" "$LOG"
grep -q "luna_exec: Luna: luna-init exec transition accepted" "$LOG"
grep -q "Luna: luna-init is running as PID 1" "$LOG"
grep -q "Luna: logical root ready; starting luna-system-runtime" "$LOG"
grep -q "luna-system-runtime: boot success confirmed" "$LOG"
grep -q "luna-system-runtime: OpenRC starting sysinit runlevel" "$LOG"
grep -q "luna-system-runtime: OpenRC sysinit runlevel ready" "$LOG"
grep -q "luna-system-runtime: OpenRC starting boot runlevel" "$LOG"
grep -q "luna-system-runtime: OpenRC boot runlevel ready" "$LOG"
grep -q "luna-system-runtime: OpenRC starting default runlevel" "$LOG"
grep -q "luna-system-runtime: OpenRC default runlevel ready" "$LOG"
grep -Fq "Starting seatd ... [ ok ]" "$LOG"
grep -Fq "Starting System Message Bus ... [ ok ]" "$LOG"
grep -Fq "Starting System login manager ... [ ok ]" "$LOG"
grep -Fq "Starting Bluetooth ... [ ok ]" "$LOG"
grep -Fq "Starting NetworkManager ... [ ok ]" "$LOG"
grep -Fq "Starting Polkit System Daemon ... [ ok ]" "$LOG"
grep -Fq "Starting RealtimeKit ... [ ok ]" "$LOG"
grep -Fq "Starting UPower ... [ ok ]" "$LOG"
grep -Fq "New seat seat0." "$LOG"
grep -q "luna-user-session: login renderer font loaded from /resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf" "$LOG"
grep -q "luna-user-session: login renderer background loaded from /resources/images/luna.png" "$LOG"
grep -q "luna-user-session: login UI graphics device /dev/dri/card0" "$LOG"
grep -q "luna-user-session: login UI acquired DRM master" "$LOG"
grep -q "luna-user-session: login UI KMS target connector=" "$LOG"
grep -q "luna-user-session: login UI keyboard/pointer input ready" "$LOG"
grep -q "luna-user-session: login UI graphics device /dev/dri/card0" "$LOG"
grep -q "luna-user-session: login UI acquired DRM master" "$LOG"
grep -q "luna-system-runtime: boot success confirmed" "$LOG"

! grep -q "Kernel panic" "$LOG"
! grep -q "session UI I/O failed" "$LOG"

sha256sum -c "$REPO_ROOT/dist/luna-0.1.0.img.sha256"

echo "Canonical Luna production OVMF verification: PASS"
echo "Serial log: $LOG"
