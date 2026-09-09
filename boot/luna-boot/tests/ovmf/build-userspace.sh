#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
OUT="${REPO_ROOT}/boot/luna-boot/tests/ovmf/out"
SYSROOT="${OUT}/system-root"

: "${LUNA_TEST_KERNEL:?Set LUNA_TEST_KERNEL to a Linux x86_64 bzImage}"

for tool in cargo rustup mksquashfs file; do
    command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

RUNTIME_TARGET="x86_64-unknown-linux-musl"
if ! rustup target list --installed | grep -qx "$RUNTIME_TARGET"; then
    echo "luna-test userspace: installing Rust target $RUNTIME_TARGET" >&2
    rustup target add "$RUNTIME_TARGET"
fi

rm -rf "$SYSROOT"
mkdir -p "$SYSROOT"/{bin,sbin,etc,dev,proc,sys,run,tmp,data,boot,home,lib,lib64,media,mnt,opt,root,srv,usr,var}
mkdir -p "$SYSROOT/usr/bin" "$SYSROOT/usr/lib" "$SYSROOT/usr/sbin" "$SYSROOT/etc/luna"

cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
cargo build --release --manifest-path "$REPO_ROOT/components/luna-init/Cargo.toml" --target "$RUNTIME_TARGET"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/luna-init/target/$RUNTIME_TARGET/release/luna-init"
if [ ! -x "$LUNA_INIT" ]; then
    LUNA_INIT="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-init"
fi
[ -x "$RUNTIME" ] || { echo "system runtime binary was not produced: $RUNTIME" >&2; exit 1; }
[ -x "$LUNA_INIT" ] || { echo "luna-init binary was not produced: $LUNA_INIT" >&2; exit 1; }

# `file(1)` reports traditional musl executables as "statically linked" and
# static PIE binaries as "static-pie linked". Both satisfy Luna's requirement
# that direct PID1 needs no external dynamic loader before the System Environment exists.
for binary in "$RUNTIME" "$LUNA_INIT"; do
    description="$(file "$binary")"
    if [[ "$description" != *"statically linked"* && "$description" != *"static-pie linked"* ]]; then
        echo "luna-test userspace: binary must be statically linked or static-PIE: $binary" >&2
        echo "  file: $description" >&2
        exit 1
    fi
done

# The System Image only needs the runtime payload at this stage. luna-init is
# loaded separately from SYSTEM/images/<version>.init by luna-boot and the kernel.
cp "$RUNTIME" "$SYSROOT/sbin/luna-system-runtime"
chmod 0755 "$SYSROOT/sbin/luna-system-runtime"
printf '/usr/bin/luna-login\n' > "$SYSROOT/etc/luna/graphical-login"
printf '/usr/bin/niri-session\n' > "$SYSROOT/etc/luna/graphical-session"

mksquashfs "$SYSROOT" "$OUT/luna-test.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null

cp "$LUNA_INIT" "$OUT/luna-test.init"
chmod 0755 "$OUT/luna-test.init"

cat > "$OUT/luna-test.toml" <<'EOF'
[image]
name = "luna"
version = "0.1.0"
format = "squashfs"
role = "normal"

[architecture]
arch = "x86_64"

[kernels]
compatible = ["test"]
EOF

echo "Built Luna QEMU userspace:"
echo "  System Image: $OUT/luna-test.squashfs"
echo "  Manifest:     $OUT/luna-test.toml"
echo "  luna-init:    $OUT/luna-test.init"
echo "  runtime:      $RUNTIME"
echo "  kernel:       $LUNA_TEST_KERNEL"
