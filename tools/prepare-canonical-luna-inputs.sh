#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INPUT_ROOT="${LUNA_CANONICAL_INPUT_ROOT:-$REPO_ROOT/dist/canonical-inputs}"
ALPINE_IMAGE="${LUNA_ALPINE_IMAGE:-alpine:3.24.2}"
NETWORK="${LUNA_DOCKER_NETWORK:-default}"
KERNEL="${LUNA_CANONICAL_KERNEL:-$REPO_ROOT/dist/kernel/current/bzImage}"

if [ "${1:-}" = --verify ]; then
    exec python3 "$REPO_ROOT/tools/canonical-luna-inputs.py" verify "$INPUT_ROOT"
fi
[ -f "$KERNEL" ] || { echo "build the Luna kernel first: $KERNEL" >&2; exit 1; }
# Never silently replace an input snapshot used by an earlier acceptance run.
[ ! -e "$INPUT_ROOT" ] || { echo "input root already exists; use --verify or choose a new LUNA_CANONICAL_INPUT_ROOT" >&2; exit 1; }
mkdir -p "$INPUT_ROOT"/{niri-apks,provider-apks,provider-root,kernel}
cp "$KERNEL" "$INPUT_ROOT/kernel/bzImage"

container_id=""
cleanup() { [ -z "$container_id" ] || docker rm -f "$container_id" >/dev/null 2>&1 || true; }
trap cleanup EXIT
container_id="$(docker create --network "$NETWORK" "$ALPINE_IMAGE" sh -ec 'apk add --no-cache openrc util-linux')"
docker start -a "$container_id"
[ "$(docker inspect --format '{{.State.ExitCode}}' "$container_id")" = 0 ]
docker export "$container_id" | gzip -n > "$INPUT_ROOT/alpine-runtime-rootfs.tar.gz"
docker image inspect --format '{{.Id}} {{json .RepoDigests}}' "$ALPINE_IMAGE" > "$INPUT_ROOT/alpine-image.txt"
docker rm "$container_id" >/dev/null
container_id=""

docker run --rm --network "$NETWORK" \
    -v "$INPUT_ROOT:/out" -v "$REPO_ROOT/tools/alpine-foundation:/recipes:ro" \
    "$ALPINE_IMAGE" sh -ec '
        packages=$(sed "/^#/d; /^$/d" /recipes/canonical-provider-packages.txt)
        services=$(sed "/^#/d; /^$/d" /recipes/desktop-service-packages.txt)
        for attempt in 1 2 3; do
            if apk update && \
               apk fetch --recursive -o /out/niri-apks niri=25.11-r2 niri-portalsconf=25.11-r2 seatd=0.9.3-r0 adwaita-fonts=50.0-r0 eudev-libs=3.2.14-r6 libudev-zero=1.0.4-r0 && \
               apk fetch --recursive -o /out/provider-apks $packages $services; then
                exit 0
            fi
            sleep 3
        done
        exit 1
    '
find "$INPUT_ROOT/provider-apks" -name '*.apk' -print0 | sort -z |
while IFS= read -r -d '' apk; do
    tar -xzf "$apk" -C "$INPUT_ROOT/provider-root" \
        --exclude='.PKGINFO' --exclude='.SIGN.*' --exclude='.INSTALL' --exclude='.trigger' 2>/dev/null
done
# The complete provider set also contains Niri's libudev-zero compatibility
# library. System service providers must retain the real eudev implementation.
tar -xzf "$INPUT_ROOT/provider-apks/eudev-libs-3.2.14-r6.apk" \
    -C "$INPUT_ROOT/provider-root" usr/lib/libudev.so.1 usr/lib/libudev.so.1.6.3 2>/dev/null

LUNA_ALPINE_IMAGE="$ALPINE_IMAGE" LUNA_ALPINE_GRAPHICS_BUILD_ROOT="$INPUT_ROOT/graphics" \
    bash "$REPO_ROOT/tools/build-alpine-foundation-graphics.sh"
LUNA_NERD_FONTS_OUT="$INPUT_ROOT/fonts" LUNA_DESKTOP_SRC="$INPUT_ROOT/sources" \
    bash "$REPO_ROOT/tools/build-nerd-fonts.sh"
python3 "$REPO_ROOT/tools/canonical-luna-inputs.py" record "$INPUT_ROOT"
python3 "$REPO_ROOT/tools/canonical-luna-inputs.py" verify "$INPUT_ROOT"
