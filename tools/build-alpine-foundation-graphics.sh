#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_ROOT="${LUNA_ALPINE_GRAPHICS_BUILD_ROOT:-$REPO_ROOT/dist/.build/alpine-foundation-graphics}"
MESA_OUT="$BUILD_ROOT/mesa/out"
NOCTALIA_OUT="$BUILD_ROOT/noctalia/out"
RUNTIME_APKS="$BUILD_ROOT/noctalia-runtime-apks"
NOCTALIA_PACKAGES="$REPO_ROOT/tools/alpine-foundation/noctalia-runtime-packages.txt"

MESA_VERSION="${LUNA_MESA_VERSION:-26.1.6}"
NOCTALIA_REF="${LUNA_NOCTALIA_REF:-v5.2.0}"
ALPINE_IMAGE="${LUNA_ALPINE_IMAGE:-alpine:3.24}"

for tool in docker sha256sum git; do
    command -v "$tool" >/dev/null || {
        echo "missing required tool: $tool" >&2
        exit 1
    }
done

mkdir -p "$BUILD_ROOT"

echo "==> Building tracked Mesa virgl recipe ($MESA_VERSION)..."
docker build \
    --build-arg "MESA_VERSION=$MESA_VERSION" \
    --build-arg "MESA_SHA256=5296b88a0f1e012e2cb9ada150a2bbadf728ca81e5a4fb2ab43c83a4d2158606" \
    -f "$REPO_ROOT/tools/alpine-foundation/mesa-virgl.Dockerfile" \
    -t "luna-alpine-mesa-virgl:$MESA_VERSION" "$REPO_ROOT/tools/alpine-foundation"

container_id="$(docker create "luna-alpine-mesa-virgl:$MESA_VERSION")"
trap 'docker rm -f "$container_id" >/dev/null 2>&1 || true' EXIT
rm -rf "$MESA_OUT"
mkdir -p "$MESA_OUT"
docker cp "$container_id:/out/." "$MESA_OUT/"
docker rm -f "$container_id" >/dev/null
trap - EXIT

test -s "$MESA_OUT/usr/local/lib/libEGL.so.1.0.0"
test -s "$MESA_OUT/usr/local/lib/libgbm.so.1.0.0"
test -d "$MESA_OUT/usr/lib/dri"

echo "==> Fetching tracked Noctalia runtime closure from $ALPINE_IMAGE..."
rm -rf "$RUNTIME_APKS"
mkdir -p "$RUNTIME_APKS"
docker run --rm \
    -v "$RUNTIME_APKS:/out" \
    -v "$NOCTALIA_PACKAGES:/packages:ro" \
    "$ALPINE_IMAGE" sh -ec '
        apk update
        apk fetch --recursive --output /out $(cat /packages)
    '

echo "==> Building tracked Noctalia recipe ($NOCTALIA_REF)..."
docker build \
    --build-arg "NOCTALIA_REF=$NOCTALIA_REF" \
    -t "luna-alpine-noctalia:${NOCTALIA_REF#v}" \
    -f "$REPO_ROOT/tools/alpine-foundation/noctalia.Dockerfile" \
    "$REPO_ROOT/tools/alpine-foundation"

container_id="$(docker create "luna-alpine-noctalia:${NOCTALIA_REF#v}")"
trap 'docker rm -f "$container_id" >/dev/null 2>&1 || true' EXIT
rm -rf "$NOCTALIA_OUT"
mkdir -p "$NOCTALIA_OUT"
docker cp "$container_id:/out/." "$NOCTALIA_OUT/"
docker rm -f "$container_id" >/dev/null
trap - EXIT

test -x "$NOCTALIA_OUT/usr/local/bin/noctalia"
test -d "$NOCTALIA_OUT/usr/local/share/noctalia/assets"
test -n "$(find "$RUNTIME_APKS" -maxdepth 1 -name '*.apk' -print -quit)"

echo
echo "Alpine graphical foundation artifacts are ready:"
echo "  Mesa:     $MESA_OUT"
echo "  Noctalia: $NOCTALIA_OUT"
echo "  Runtime:  $RUNTIME_APKS"

