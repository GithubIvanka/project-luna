#!/usr/bin/env bash
set -euo pipefail
# Sources and SDK must be prepared separately; existing checkouts are never reset.
repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
work="${1:?usage: build-luna-desktop-apps-musl.sh WORK SDK_IMAGE}"
sdk="${2:?an explicit frozen Alpine SDK image is required}"
work="$(realpath "$work")"
case "$work" in "$repo/target/"*) ;; *) echo 'work must be inside repository target' >&2; exit 1 ;; esac
[ "$(git -C "$work/upstream/yazi" rev-parse HEAD)" = 8dd895c695a5950330c2623eb43debf323b60654 ]
[ "$(git -C "$work/upstream/ghostty" rev-parse HEAD)" = 332b2aefc6e72d363aa93ab6ecfc86eeeeb5ed28 ]
git -C "$work/upstream/yazi" diff --exit-code
git -C "$work/upstream/ghostty" diff --exit-code
[ "$("$work/upstream/zig-x86_64-linux-0.15.2/zig" version)" = 0.15.2 ]
relative="${work#"$repo"/}"
sdk_id="$(docker image inspect --format '{{.Id}}' "$sdk")"
sdk_packages="$(docker run --rm --entrypoint sha256sum "$sdk_id" /lib/apk/db/installed | cut -d' ' -f1)"
# Overlay one reviewed source patch in the build container. The pinned upstream
# checkout stays byte-identical and clean on the host; no reset/copy is needed.
patch --batch --forward --fuzz=0 --output="$work/ghostty-wayland-luna.zig" \
    "$work/upstream/ghostty/src/apprt/gtk/winproto/wayland.zig" \
    "$repo/tools/alpine-foundation/ghostty-wayland-app-id.patch"
python3 - "$work" "$repo" "$sdk_id" "$sdk_packages" <<'PY'
import hashlib, json, sys
from pathlib import Path
work, repo = map(Path, sys.argv[1:3])
def digest(path):
    with path.open('rb') as source: return hashlib.file_digest(source, 'sha256').hexdigest()
record = {'format': 1, 'sdk_image_id': sys.argv[3], 'sdk_package_database_sha256': sys.argv[4],
    'ghostty_revision': '332b2aefc6e72d363aa93ab6ecfc86eeeeb5ed28',
    'yazi_revision': '8dd895c695a5950330c2623eb43debf323b60654',
    'ghostty_patch_sha256': digest(repo/'tools/alpine-foundation/ghostty-wayland-app-id.patch'),
    'ghostty_overlay_sha256': digest(work/'ghostty-wayland-luna.zig'),
    'luna_files_sources': {}}
# Клиент открытия файлов зависит от локальных компонентов; учитываем их исходники.
import importlib.util
spec = importlib.util.spec_from_file_location('desktop_packager', repo/'tools/prepare-luna-desktop-bundles.py')
packager = importlib.util.module_from_spec(spec); spec.loader.exec_module(packager)
record['luna_files_sources'] = packager.files_source_hashes()
(work/'desktop-build-pending.json').write_text(json.dumps(record, indent=2)+'\n')
PY
docker run --rm --user "$(id -u):$(id -g)" \
    -v "$repo:/workspace" -w /workspace \
    --mount "type=bind,src=$work/ghostty-wayland-luna.zig,dst=/workspace/$relative/upstream/ghostty/src/apprt/gtk/winproto/wayland.zig,readonly" \
    -e CARGO_HOME="/workspace/$relative/cargo-home" -e CARGO_BUILD_JOBS=4 \
    -e ZIG_GLOBAL_CACHE_DIR="/workspace/$relative/zig-cache" \
    --entrypoint sh "$sdk_id" -c '
        set -eu
        work="$1"
        # Проверяем в настоящем SDK и файловый движок, и клиент/посредник открытия.
        CARGO_TARGET_DIR="$work/musl-build" cargo test --locked --offline --no-default-features -p luna-files
        CARGO_TARGET_DIR="$work/musl-build" cargo test --locked --offline -p luna-app-runtime -p luna-system-runtime
        CARGO_TARGET_DIR="$work/musl-build" cargo build --release --locked --offline -p luna-files
        CARGO_TARGET_DIR="$work/yazi-musl" cargo build --release --locked --offline --manifest-path "$work/upstream/yazi/Cargo.toml" -p yazi-fm -p yazi-cli
        cd "$work/upstream/ghostty"
        "$work/upstream/zig-x86_64-linux-0.15.2/zig" build -j4 -fsys=gtk4-layer-shell -Doptimize=ReleaseFast -Dapp-runtime=gtk -Di18n=false -Dversion-string=1.3.1 -p "$work/ghostty-musl/usr"
    ' sh "/workspace/$relative"
git -C "$work/upstream/ghostty" diff --exit-code
python3 - "$work" "$repo" <<'PY'
import hashlib, json, sys
from pathlib import Path
work, repo = map(Path, sys.argv[1:3])
def digest(path):
    with path.open('rb') as source: return hashlib.file_digest(source, 'sha256').hexdigest()
record = json.loads((work/'desktop-build-pending.json').read_text())
assert record['ghostty_patch_sha256'] == digest(repo/'tools/alpine-foundation/ghostty-wayland-app-id.patch')
assert record['ghostty_overlay_sha256'] == digest(work/'ghostty-wayland-luna.zig')
assert all(digest(repo/path) == value for path,value in record['luna_files_sources'].items())
record['artifacts'] = {name: digest(work/path) for name, path in {
    'ghostty': 'ghostty-musl/usr/bin/ghostty', 'luna-files': 'musl-build/release/luna-files',
    'yazi': 'yazi-musl/release/yazi', 'ya': 'yazi-musl/release/ya'}.items()}
temporary = work/'ghostty-musl/build-provenance.tmp'
temporary.write_text(json.dumps(record, indent=2)+'\n')
temporary.replace(work/'ghostty-musl/build-provenance.json')
(work/'desktop-build-pending.json').unlink()
print('Native desktop build receipt: PASS')
PY
