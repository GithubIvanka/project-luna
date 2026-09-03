#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-uefi"
PROFILE="${LUNA_BUILD_PROFILE:-release}"

command -v cargo >/dev/null 2>&1 || { echo "Ошибка: не найден cargo." >&2; exit 1; }
command -v rustup >/dev/null 2>&1 || { echo "Ошибка: не найден rustup." >&2; exit 1; }

if ! rustup target list --installed | grep -qx "$TARGET"; then
    echo "Устанавливается Rust target: $TARGET"
    rustup target add "$TARGET"
fi

cd "$REPO_ROOT/boot/luna-boot"
echo "Сборка luna-boot для $TARGET ($PROFILE)..."
if [ "$PROFILE" = "release" ]; then
    cargo build --release --target "$TARGET"
    ARTIFACT="target/$TARGET/release/luna-boot.efi"
else
    cargo build --target "$TARGET"
    ARTIFACT="target/$TARGET/debug/luna-boot.efi"
fi

[ -f "$ARTIFACT" ] || { echo "Ошибка: результат сборки не найден: $ARTIFACT" >&2; exit 1; }

printf '\nГотово. luna-boot: %s\n' "$REPO_ROOT/boot/luna-boot/$ARTIFACT"
