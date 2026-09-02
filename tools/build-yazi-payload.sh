#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"
YAZI_TAG="${LUNA_YAZI_TAG:-v26.9.1}"

mkdir -p "$ROOT" "$SRC"
fetch_git() {
  local url="$1" ref="$2" dir="$3"
  if [ ! -d "$dir/.git" ]; then git clone --filter=blob:none --no-tags "$url" "$dir"; fi
  git -C "$dir" fetch --depth 1 origin "$ref"
  git -C "$dir" checkout --force FETCH_HEAD
git -C "$dir" clean -fdx >/dev/null
}

fetch_git https://github.com/sxyazi/yazi.git "$YAZI_TAG" "$SRC/yazi"
(
  cd "$SRC/yazi"
  cargo build --release -p yazi-fm -p yazi-cli -j "$JOBS"
)

install -Dm0755 "$SRC/yazi/target/release/yazi" "$ROOT/usr/bin/yazi"
install -Dm0755 "$SRC/yazi/target/release/ya" "$ROOT/usr/bin/ya"

mkdir -p "$ROOT/etc/yazi" "$ROOT/usr/share/applications"
cat > "$ROOT/etc/yazi/luna.toml" <<'EOF'
[mgr]
ratio = [1, 2, 3]
show_hidden = true
show_symlink = true
sort_by = "alphabetical"
sort_sensitive = false
sort_reverse = false
linemode = "size"

[preview]
wrap = "yes"
tab_size = 2
max_width = 1200
max_height = 900

[opener]
edit = [{ run = "${EDITOR:-/usr/bin/nvim} %s", block = true, for = "unix" }]
open = [{ run = "xdg-open %s", orphan = true, desc = "Open" }]
reveal = [{ run = "luna-files --reveal %s", orphan = true, desc = "Reveal in Luna Files" }]
EOF

cat > "$ROOT/usr/share/applications/yazi.desktop" <<'EOF'
[Desktop Entry]
Name=Luna Files (Yazi CLI)
Comment=Project Luna file manager terminal interface
Exec=/usr/bin/ghostty -e /usr/bin/yazi
Terminal=false
Type=Application
Categories=System;FileManager;
EOF

cat > "$ROOT/etc/luna/files.toml" <<EOF
[file_manager]
name = "Luna Files"
engine = "yazi"
engine_version = "$YAZI_TAG"
cli = "/usr/bin/yazi"
cli_helper = "/usr/bin/ya"
gui = "/usr/bin/luna-files"
backend_model = "yazi-core"

[paths]
user_home = "/data/users/luna/home"
user_data = "/data/users/luna/data"
removable_media = "/run/media/luna"
EOF

# Keep a machine-readable pin beside the image, useful for upgrades and CI.
cat > "$ROOT/etc/luna/yazi.version" <<EOF
$YAZI_TAG
EOF

echo "Built Yazi payload: $ROOT/usr/bin/yazi ($YAZI_TAG)"
