#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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

cd "$REPO_ROOT"
echo "Сборка workspace crate: $component"
if [ "$#" -eq 0 ]; then
    cargo build -p "$component"
else
    cargo build -p "$component" "$@"
fi

echo "Готово: crate $component"
