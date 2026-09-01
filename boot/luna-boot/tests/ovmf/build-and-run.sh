#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
OVMF_DIR="${REPO_ROOT}/boot/luna-boot/tests/ovmf"
OUT="${OVMF_DIR}/out"

"${OVMF_DIR}/build-userspace.sh"

export LUNA_TEST_INITRD="$OUT/luna-test-initramfs.img"
export LUNA_TEST_SQUASHFS="$OUT/luna-test.squashfs"

exec "${OVMF_DIR}/run.sh"
