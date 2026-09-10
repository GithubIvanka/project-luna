#!/usr/bin/env bash
set -euo pipefail

MESON_FILE="${1:?usage: tools/patch-noctalia-greeter.sh <meson.build>}"

[ -f "$MESON_FILE" ] || { echo "missing Meson file: $MESON_FILE" >&2; exit 1; }

# The upstream project enables host-native CPU tuning by default. Project Luna
# builds a portable desktop root, so remove that exact two-flag line before
# invoking Meson.
sed -i "/^[[:space:]]*'-march=native', '-mtune=native',\$/d" "$MESON_FILE"
