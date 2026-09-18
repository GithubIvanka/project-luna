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
mkdir -p "$SYSROOT"/{apps,drivers,firmware,libs,config,resources}
mkdir -p "$SYSROOT/resources"/{fonts,icons,themes,cursors,locales,translations}
mkdir -p "$SYSROOT/apps/luna-system-runtime" "$SYSROOT/apps/luna-user-session" "$SYSROOT/config/luna"
mkdir -p "$SYSROOT/libs/loader"

cargo build --release -p luna-system-runtime --target "$RUNTIME_TARGET"
cargo build --release --manifest-path "$REPO_ROOT/components/system/luna-init/Cargo.toml" --target "$RUNTIME_TARGET"
RUNTIME="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-system-runtime"
LUNA_INIT="$REPO_ROOT/components/system/luna-init/target/$RUNTIME_TARGET/release/luna-init"
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

# The System Image follows the canonical Luna resource layout. The
# luna-system-runtime and UserSession binaries are immutable app resources.
cp "$RUNTIME" "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"
chmod 0755 "$SYSROOT/apps/luna-system-runtime/luna-system-runtime"

cargo build --release -p luna-user-session --bin luna-user-session --target "$RUNTIME_TARGET"
SESSION="$REPO_ROOT/target/$RUNTIME_TARGET/release/luna-user-session"
[ -x "$SESSION" ] || { echo "UserSession handoff binary was not produced: $SESSION" >&2; exit 1; }
cp "$SESSION" "$SYSROOT/apps/luna-user-session/luna-user-session"
chmod 0755 "$SYSROOT/apps/luna-user-session/luna-user-session"
printf "%s\n" "/usr/bin/luna-user-session --handoff" > "$SYSROOT/config/luna/graphical-session"
cat > "$SYSROOT/config/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
greeter:x:995:995:Luna Greeter:/run/greetd:/bin/sh
luna:x:1000:1000:Luna User:/home/luna:/bin/sh
EOF
cat > "$SYSROOT/config/group" <<'EOF'
root:x:0:
shadow:x:42:
greeter:x:995:
luna:x:1000:
EOF
cat > "$SYSROOT/config/shadow" <<'EOF'
root:!*:0:0:99999:7:::
greeter:!*:0:0:99999:7:::
luna:!*:0:0:99999:7:::
EOF
chmod 0644 "$SYSROOT/config/passwd" "$SYSROOT/config/group"
chmod 0600 "$SYSROOT/config/shadow"
printf "%s\n" "passwd: files" "group: files" "shadow: files" "gshadow: files" "hosts: files dns" "services: files" "networks: files" "protocols: files" > "$SYSROOT/config/nsswitch.conf"
chmod 0644 "$SYSROOT/config/nsswitch.conf"

LOADER_SOURCE="$(readlink -f /lib64/ld-linux-x86-64.so.2)"
[ -f "$LOADER_SOURCE" ] || { echo "luna-test userspace: ELF loader is unavailable" >&2; exit 1; }
cp -L "$LOADER_SOURCE" "$SYSROOT/libs/loader/ld-linux-x86-64.so.2"

mksquashfs "$SYSROOT" "$OUT/luna-0.1.0.squashfs" -noappend -comp zstd -all-root -no-xattrs >/dev/null
cp "$LUNA_INIT" "$OUT/luna-test.init"
chmod 0755 "$OUT/luna-test.init"

cat > "$OUT/luna-0.1.0.toml" <<'EOF'
[image]
name = "luna"
version = "0.1.0"
format = "squashfs"
role = "normal"

[architecture]
arch = "x86_64"

[init]
compatible = ["0.1.0"]

[bootstrap]
critical = ["/apps/luna-system-runtime/luna-system-runtime"]
EOF

echo "Built Luna QEMU userspace:"
echo "  System Image: $OUT/luna-0.1.0.squashfs"
echo "  Manifest:     $OUT/luna-0.1.0.toml"
echo "  luna-init:    $OUT/luna-test.init"
echo "  runtime:      $RUNTIME"
echo "  kernel:       $LUNA_TEST_KERNEL"
