#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_KERNEL_OUT:-${REPO_ROOT}/dist/kernel}"
VERSION="${LUNA_KERNEL_VERSION:-7.2.4}"
JOBS="${LUNA_KERNEL_JOBS:-$(nproc)}"
LLVM_MODE="${LUNA_KERNEL_LLVM:-1}"
SRC="${OUT}/linux-${VERSION}"
TARBALL="${OUT}/linux-${VERSION}.tar.xz"
URL="https://www.kernel.org/pub/linux/kernel/v7.x/linux-${VERSION}.tar.xz"
CONFIG_FRAGMENT="${REPO_ROOT}/kernel/luna-x86_64.config"
OVERLAY="${REPO_ROOT}/tools/apply-luna-kernel-overlay.sh"
PROGRESS_BIN="${REPO_ROOT}/target/release/build-progress"
LOG_DIR="${OUT}/logs"
LOG_FILE="${LUNA_KERNEL_LOG:-${LOG_DIR}/kernel-${VERSION}-$(date +%Y%m%d-%H%M%S).log}"

for tool in curl tar make cargo; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

LLVM_PREFIX=""
if [ "$LLVM_MODE" = "1" ]; then
    candidates=()
    if [ -n "${LUNA_LLVM_BIN:-}" ]; then
        candidates+=("$LUNA_LLVM_BIN")
    fi
    for candidate in /usr/lib/llvm-*/bin /usr/local/opt/llvm/bin; do
        candidates+=("$candidate")
    done

    for candidate in "${candidates[@]}"; do
        [ -n "$candidate" ] || continue
        if [ -x "$candidate/clang" ] && [ -x "$candidate/ld.lld" ] && [ -x "$candidate/llvm-ar" ]; then
            LLVM_PREFIX="$(cd "$candidate" && pwd)"
            break
        fi
    done

    if [ -z "$LLVM_PREFIX" ] && command -v clang >/dev/null 2>&1 && command -v ld.lld >/dev/null 2>&1 && command -v llvm-ar >/dev/null 2>&1; then
        LLVM_PREFIX="$(dirname "$(command -v clang)")"
    fi

    if [ -z "$LLVM_PREFIX" ]; then
        echo "cannot locate a complete LLVM toolchain (clang, ld.lld, llvm-ar)" >&2
        echo "set LUNA_LLVM_BIN only if LLVM is installed outside standard paths" >&2
        exit 1
    fi

    export PATH="$LLVM_PREFIX:$PATH"
fi

[ -f "$CONFIG_FRAGMENT" ] || { echo "missing kernel config: $CONFIG_FRAGMENT" >&2; exit 1; }
[ -f "$OVERLAY" ] || { echo "missing kernel overlay: $OVERLAY" >&2; exit 1; }

ensure_progress_tool() {
    local root_manifest="${REPO_ROOT}/Cargo.toml"
    local tool_manifest="${REPO_ROOT}/tools/build-progress/Cargo.toml"
    if [ ! -x "$PROGRESS_BIN" ] || [ "$tool_manifest" -nt "$PROGRESS_BIN" ] || [ "$root_manifest" -nt "$PROGRESS_BIN" ] || find "${REPO_ROOT}/tools/build-progress/src" -type f -newer "$PROGRESS_BIN" -print -quit | grep -q .; then
        echo "Building Luna build-progress tool..."
        cargo build --quiet --release -p luna-build-progress
    fi
    [ -x "$PROGRESS_BIN" ] || { echo "missing build progress executable: $PROGRESS_BIN" >&2; exit 1; }
}

if command -v rustc >/dev/null; then
    RUST_SYSROOT="$(rustc --print sysroot)"
    AUTO_RUST_LIB_SRC="${RUST_SYSROOT}/lib/rustlib/src/rust/library"
    if [ -d "$AUTO_RUST_LIB_SRC" ] && [ -z "${RUST_LIB_SRC:-}" ]; then
        export RUST_LIB_SRC="$AUTO_RUST_LIB_SRC"
    fi
fi

mkdir -p "$OUT" "$LOG_DIR"
if [ ! -d "$SRC" ]; then
    if [ ! -f "$TARBALL" ]; then
        curl --fail --location --retry 3 --output "$TARBALL" "$URL"
    fi
    tar -xJf "$TARBALL" -C "$OUT"
fi

cd "$SRC"

if [ -e .luna-overlay-applied ]; then
    echo "refusing to reuse an already overlaid kernel source: $SRC" >&2
    echo "remove $SRC or build with a fresh LUNA_KERNEL_OUT" >&2
    exit 1
fi

bash "$OVERLAY" "$SRC" "$REPO_ROOT"
touch .luna-overlay-applied

MAKE=(make)
if [ "$LLVM_MODE" = "1" ]; then
    MAKE+=(LLVM="$LLVM_PREFIX/")
fi
MAKE+=(O="$SRC/build" ARCH=x86_64)

if ! "${MAKE[@]}" rustavailable; then
    echo "Project Luna requires a Rust toolchain accepted by the Linux kernel build system." >&2
    echo "Install/enable rust-src and bindgen as described by Documentation/rust/quick-start.rst." >&2
    exit 1
fi

"${MAKE[@]}" x86_64_defconfig
KCONFIG_CONFIG="$SRC/build/.config" "$SRC/scripts/kconfig/merge_config.sh" -m -r "$SRC/build/.config" "$CONFIG_FRAGMENT"
"${MAKE[@]}" olddefconfig

if ! grep -q '^CONFIG_RUST=y$' "$SRC/build/.config"; then
    echo "Project Luna requires CONFIG_RUST=y after olddefconfig" >&2
    echo "Check RUST_IS_AVAILABLE and other CONFIG_RUST dependencies above." >&2
    exit 1
fi

"${MAKE[@]}" rustavailable

TOTAL="$(${MAKE[@]} -n bzImage modules 2>/dev/null | awk '
    /(^|[[:space:]])(clang|gcc|rustc|ld\.lld|ld|llvm-ar|ar|as|objcopy|objdump|strip)([[:space:]]|$)/ { count++ }
    END { print count + 0 }
')"

ensure_progress_tool

"$PROGRESS_BIN" \
    --label "Linux ${VERSION}" \
    --log "$LOG_FILE" \
    --total "$TOTAL" \
    -- "${MAKE[@]}" -j"$JOBS" bzImage modules

KERNEL_RELEASE="$("${MAKE[@]}" -s kernelrelease)"
mkdir -p "$OUT/$KERNEL_RELEASE"
cp "$SRC/build/arch/x86/boot/bzImage" "$OUT/$KERNEL_RELEASE/bzImage"
cp "$SRC/build/System.map" "$OUT/$KERNEL_RELEASE/System.map"
cp "$SRC/build/.config" "$OUT/$KERNEL_RELEASE/config"
printf '%s\n' "$KERNEL_RELEASE" > "$OUT/$KERNEL_RELEASE/release"
ln -sfn "$KERNEL_RELEASE" "$OUT/current"

echo "Built Project Luna kernel: $OUT/$KERNEL_RELEASE/bzImage"
