#!/usr/bin/env bash
set -euo pipefail

artifact="${1:-components/luna-init/target/x86_64-unknown-linux-musl/release/luna-init}"

if [[ ! -f "$artifact" ]]; then
    printf 'luna-init artifact not found: %s\n' "$artifact" >&2
    printf 'build it first with: cargo build --manifest-path components/luna-init/Cargo.toml --release\n' >&2
    exit 1
fi

if ! command -v readelf >/dev/null 2>&1; then
    printf 'readelf is required for static ELF validation\n' >&2
    exit 1
fi

if ! readelf -h "$artifact" | grep -Eq 'Class:[[:space:]]+ELF64'; then
    printf 'luna-init is not ELF64: %s\n' "$artifact" >&2
    exit 1
fi

if ! readelf -h "$artifact" | grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64'; then
    printf 'luna-init is not x86_64: %s\n' "$artifact" >&2
    exit 1
fi

if readelf -l "$artifact" | grep -q 'Requesting program interpreter'; then
    printf 'luna-init is dynamically linked: interpreter requested\n' >&2
    exit 1
fi

if ! readelf -d "$artifact" 2>/dev/null | grep -q 'There is no dynamic section'; then
    printf 'luna-init still contains a dynamic section\n' >&2
    exit 1
fi

printf 'luna-init static ELF validation passed: %s\n' "$artifact"
