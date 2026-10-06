#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT_DIR}/tests/ovmf/out"

export OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}"
export OVMF_VARS="${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}"
export LUNA_QEMU_DISPLAY="${LUNA_QEMU_DISPLAY:-gtk}"
export LUNA_QEMU_SERIAL_LOG="${LUNA_QEMU_SERIAL_LOG:-${OUT}/serial.log}"

if [[ -z "${DISPLAY:-}" && -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/wayland-0" ]]; then
    export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
    export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
fi

: "${LUNA_TEST_KERNEL:?Set LUNA_TEST_KERNEL to a Linux x86_64 bzImage}"

exec "${ROOT_DIR}/tests/ovmf/run.sh"
