#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-uefi"
PROFILE="${LUNA_BUILD_PROFILE:-release}"
LOG_DIR="${REPO_ROOT}/dist/logs"
LOG_FILE="${LUNA_BOOT_LOG:-${LOG_DIR}/luna-boot-$(date +%Y%m%d-%H%M%S).log}"

command -v cargo >/dev/null 2>&1 || { echo "Ошибка: не найден cargo." >&2; exit 1; }
command -v rustup >/dev/null 2>&1 || { echo "Ошибка: не найден rustup." >&2; exit 1; }

resolve_target_dir() {
    local manifest="$1"
    local metadata
    metadata="$(cargo metadata --manifest-path "$manifest" --no-deps --format-version 1)" || {
        echo "Ошибка: не удалось получить Cargo metadata для $manifest" >&2
        exit 1
    }

    printf '%s\n' "$metadata" \
        | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' \
        | head -n 1
}

if ! rustup target list --installed | grep -qx "$TARGET"; then
    echo "Устанавливается Rust target: $TARGET"
    rustup target add "$TARGET"
fi

cd "$REPO_ROOT/boot/luna-boot"
mkdir -p "$LOG_DIR"

echo "Сборка luna-boot для $TARGET ($PROFILE)..."
if [ "$PROFILE" = "release" ]; then
    COMMAND=(cargo build --release --target "$TARGET")
    ARTIFACT="target/$TARGET/release/luna-boot.efi"
else
    COMMAND=(cargo build --target "$TARGET")
    ARTIFACT="target/$TARGET/debug/luna-boot.efi"
fi

ROOT_MANIFEST="${REPO_ROOT}/Cargo.toml"
PROGRESS_TARGET_DIR="$(resolve_target_dir "$ROOT_MANIFEST")"
[ -n "$PROGRESS_TARGET_DIR" ] || {
    echo "Ошибка: Cargo metadata не вернул target directory для build-progress." >&2
    exit 1
}
PROGRESS_BIN="${PROGRESS_TARGET_DIR}/release/build-progress"
PROGRESS_MANIFEST="${REPO_ROOT}/tools/build-progress/Cargo.toml"

ensure_progress_tool() {
    if [ ! -x "$PROGRESS_BIN" ] \
        || [ "$PROGRESS_MANIFEST" -nt "$PROGRESS_BIN" ] \
        || [ "$ROOT_MANIFEST" -nt "$PROGRESS_BIN" ] \
        || find "${REPO_ROOT}/tools/build-progress/src" -type f -newer "$PROGRESS_BIN" -print -quit | grep -q .; then
        echo "Building Luna build-progress tool..."
        cargo build --manifest-path "$ROOT_MANIFEST" --quiet --release -p luna-build-progress
    fi
    [ -x "$PROGRESS_BIN" ] || {
        echo "Ошибка: не найден build progress executable: $PROGRESS_BIN" >&2
        exit 1
    }
}

ensure_progress_tool
"$PROGRESS_BIN" \
    --label "luna-boot" \
    --log "$LOG_FILE" \
    --action "Finished" \
    -- \
    "${COMMAND[@]}"

[ -f "$ARTIFACT" ] || { echo "Ошибка: результат сборки не найден: $ARTIFACT" >&2; exit 1; }

printf '\nГотово. luna-boot: %s\n' "$REPO_ROOT/boot/luna-boot/$ARTIFACT"
