#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_root/target/luna-application-bundles}"
mkdir -p -- "$output_dir"
output_dir="$(cd -- "$output_dir" && pwd)"
output="$output_dir/luna-cli.lbp"
if [[ -e "$output" || -L "$output" ]]; then
    echo "Bundle output already exists: $output" >&2
    exit 1
fi

cd -- "$repo_root"
cargo build --locked --offline --target x86_64-unknown-linux-musl -p luna-cli
payload="$(mktemp -d "$output_dir/.luna-cli-payload-XXXXXX")"
trap 'rm -rf -- "$payload"' EXIT
mkdir -- "$payload/bin" "$payload/share"
install -m 0755 target/x86_64-unknown-linux-musl/debug/luna "$payload/bin/luna"
printf '%s\n' 'Project Luna CLI 0.1.0 — native musl application Bundle.' > "$payload/share/about.txt"
target/x86_64-unknown-linux-musl/debug/luna bundle pack \
    examples/bundles/luna-cli/manifest.toml "$payload" "$output"
target/x86_64-unknown-linux-musl/debug/luna bundle inspect "$output"
