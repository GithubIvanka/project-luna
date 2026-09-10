#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROGRESS="${REPO_ROOT}/tools/build-progress.py"
LOG_DIR="${REPO_ROOT}/dist/logs"

component="${1:-}"
if [ -z "$component" ]; then
    echo "Использование: tools/build-component.sh <crate> [cargo-args...]" >&2
    echo "Пример: tools/build-component.sh luna-system-runtime --release" >&2
    exit 2
fi
shift

command -v cargo >/dev/null 2>&1 || { echo "Ошибка: не найден cargo." >&2; exit 1; }
[ -f "$PROGRESS" ] || { echo "Ошибка: не найден build progress helper: $PROGRESS" >&2; exit 1; }

case "$component" in
    luna-boot)
        echo "Для luna-boot используйте tools/build-luna-boot.sh" >&2
        exit 2
        ;;
    *)
        ;;
esac

cd "$REPO_ROOT"
mkdir -p "$LOG_DIR"
LOG_FILE="${LUNA_COMPONENT_LOG:-${LOG_DIR}/${component}-$(date +%Y%m%d-%H%M%S).log}"

echo "Сборка workspace crate: $component"

if [ "$#" -eq 0 ]; then
    COMMAND=(cargo build -p "$component")
else
    COMMAND=(cargo build -p "$component" "$@")
fi

python3 "$PROGRESS" \
    --label "Cargo ${component}" \
    --log "$LOG_FILE" \
    --action-regex '^\s*Compiling\s+' \
    -- "${COMMAND[@]}"

echo "Готово: crate $component"
