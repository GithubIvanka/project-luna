#!/usr/bin/env bash
set -euo pipefail

key_file="$HOME/.local/share/opencode/auth.json"
if [ ! -f "$key_file" ]; then
  echo "OpenCode OpenRouter credentials not found: $key_file" >&2
  exit 1
fi

OPENROUTER_API_KEY="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["openrouter"]["key"])' "$key_file")"
if [ -z "$OPENROUTER_API_KEY" ]; then
  echo "OpenCode OpenRouter credential is empty" >&2
  exit 1
fi

export OPENROUTER_API_KEY
exec "$HOME/.local/bin/opencode" "$@"
