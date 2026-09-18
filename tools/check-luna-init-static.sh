#!/usr/bin/env bash
set -euo pipefail

artifact="${1:-components/system/luna-init/target/x86_64-unknown-linux-musl/release/luna-init}"

if [[ ! -f "$artifact" ]]; then
    printf 'luna-init artifact not found: %s\n' "$artifact" >&2
    printf 'build it first with: cargo build --manifest-path components/system/luna-init/Cargo.toml --release\n' >&2
    exit 1
fi

if ! command -v readelf >/dev/null 2>&1; then
    printf 'readelf is required for static ELF validation\n' >&2
    exit 1
fi

header="$(readelf -h "$artifact")"
program_headers="$(readelf -l "$artifact")"
dynamic="$(readelf -d "$artifact" 2>/dev/null || true)"

if ! grep -q 'ELF64' <<<"$header"; then
    printf 'luna-init is not ELF64: %s\n' "$artifact" >&2
    exit 1
fi

if ! grep -q 'X86-64' <<<"$header"; then
    printf 'luna-init is not x86_64: %s\n' "$artifact" >&2
    exit 1
fi

if grep -Eq '(^|[[:space:]])INTERP([[:space:]]|$)' <<<"$program_headers"; then
    printf 'luna-init is dynamically linked: PT_INTERP present\n' >&2
    exit 1
fi

if grep -q '(NEEDED)' <<<"$dynamic"; then
    printf 'luna-init has dynamic shared-library dependencies\n' >&2
    grep '(NEEDED)' <<<"$dynamic" >&2 || true
    exit 1
fi

if grep -Eq '\(RPATH\)|\(RUNPATH\)' <<<"$dynamic"; then
    printf 'luna-init contains runtime library search paths\n' >&2
    exit 1
fi

printf 'luna-init static ELF validation passed: %s\n' "$artifact"
