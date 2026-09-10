#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROGRESS_BIN="${REPO_ROOT}/target/release/build-progress"
LOG_DIR="${REPO_ROOT}/dist/logs"

component="${1:-}"
if [ -z "$component" ]; then
    echo "Использование: tools/build-component.sh <crate> [cargo-args...]" >&2
    echo "Пример: tools/build-component.sh luna-system-runtime --release" >&2
    exit 2
fi
shift

command -v cargo >/dev/null 2>&1 || { echo "Ошибка: не найден cargo." >&2; exit 1; }

case "$component" in
    luna-boot)
        echo "Для luna-boot используйте tools/build-luna-boot.sh" >&2
        exit 2
        ;;
    *)
        ;;
esac

ensure_progress_tool() {
    local root_manifest="${REPO_ROOT}/Cargo.toml"
    local tool_manifest="${REPO_ROOT}/tools/build-progress/Cargo.toml"
    if [ ! -x "$PROGRESS_BIN" ] \
        || [ "$tool_manifest" -nt "$PROGRESS_BIN" ] \
        || [ "$root_manifest" -nt "$PROGRESS_BIN" ] \
        || find "${REPO_ROOT}/tools/build-progress/src" -type f -newer "$PROGRESS_BIN" -print -quit | grep -q .; then
        echo "Building Luna build-progress tool..."
        cargo build --quiet --release -p luna-build-progress
    fi
    [ -x "$PROGRESS_BIN" ] || {
        echo "Ошибка: не найден build progress executable: $PROGRESS_BIN" >&2
        exit 1
    }
}

cd "$REPO_ROOT"
mkdir -p "$LOG_DIR"
LOG_FILE="${LUNA_COMPONENT_LOG:-${LOG_DIR}/${component}-$(date +%Y%m%d-%H%M%S).log}"

echo "Сборка workspace crate: $component"

if [ "$#" -eq 0 ]; then
    COMMAND=(cargo build -p "$component")
else
    COMMAND=(cargo build -p "$component" "$@")
fi

ensure_progress_tool
"$PROGRESS_BIN" \
    --label "Cargo ${component}" \
    --log "$LOG_FILE" \
    -- \
    "${COMMAND[@]}"

echo "Готово: crate $component"
