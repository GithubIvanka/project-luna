#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-x86_64-unknown-linux-musl}"

for binary in \
    "$ROOT/target/$TARGET/release/luna-system-runtime" \
    "$ROOT/target/$TARGET/release/luna-user-session" \
    "$ROOT/components/system/luna-init/target/$TARGET/release/luna-init"; do
    [ -x "$binary" ] || { echo "missing: $binary" >&2; exit 1; }
    info="$(file "$binary")"
    printf '%s\n' "$info"
    grep -Eq 'statically linked|static-pie linked' <<<"$info" || {
        echo "not statically linked: $binary" >&2
        exit 1
    }
done

echo "Luna early userspace binaries: static check OK"
