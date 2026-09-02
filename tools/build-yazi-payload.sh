#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${LUNA_DESKTOP_ROOT:-${REPO_ROOT}/dist/desktop-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"
YAZI_TAG="${LUNA_YAZI_TAG:-v26.9.1}"
YAZI_COMMIT="${LUNA_YAZI_COMMIT:-8dd895c695a5950330c2623eb43debf323b60654}"

mkdir -p "$ROOT" "$SRC"
fetch_git() {
  local url="$1" ref="$2" dir="$3"
  if [ ! -d "$dir/.git" ]; then git clone --filter=blob:none --no-tags "$url" "$dir"; fi
  git -C "$dir" fetch --depth 1 origin "$ref"
  git -C "$dir" checkout --force FETCH_HEAD
  git -C "$dir" clean -fdx >/dev/null
}

fetch_git https://github.com/sxyazi/yazi.git "$YAZI_TAG" "$SRC/yazi"
ACTUAL_YAZI_COMMIT="$(git -C "$SRC/yazi" rev-parse HEAD)"
[ "$ACTUAL_YAZI_COMMIT" = "$YAZI_COMMIT" ] || {
  echo "Yazi tag $YAZI_TAG resolved to unexpected commit: $ACTUAL_YAZI_COMMIT (expected $YAZI_COMMIT)" >&2
  exit 1
}
(
  cd "$SRC/yazi"
  cargo build --release -p yazi-fm -p yazi-cli -j "$JOBS"
)

install -Dm0755 "$SRC/yazi/target/release/yazi" "$ROOT/usr/bin/yazi"
install -Dm0755 "$SRC/yazi/target/release/ya" "$ROOT/usr/bin/ya"

# Luna Files is a native Wayland GUI today and will progressively consume the
# same yazi-core/yazi-fs/yazi-vfs model instead of maintaining a second file
# operation implementation.
cargo build --release -p luna-files -j "$JOBS"
install -Dm0755 "$REPO_ROOT/target/release/luna-files" "$ROOT/usr/bin/luna-files"

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

cat > "$ROOT/usr/share/applications/luna-files.desktop" <<'EOF'
[Desktop Entry]
Name=Luna Files
Comment=Project Luna graphical file manager
Exec=/usr/bin/luna-files
Icon=system-file-manager
Terminal=false
Type=Application
Categories=System;FileManager;
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
engine_commit = "$ACTUAL_YAZI_COMMIT"
cli = "/usr/bin/yazi"
cli_helper = "/usr/bin/ya"
gui = "/usr/bin/luna-files"
backend_model = "yazi-core"

[paths]
user_home = "/data/users/luna/home"
user_data = "/data/users/luna/data"
removable_media = "/run/media/luna"
EOF

cat > "$ROOT/etc/luna/yazi.version" <<EOF
version=$YAZI_TAG
commit=$ACTUAL_YAZI_COMMIT
EOF

# The GUI and Yazi are glibc user-space applications. Copy their complete
# shared-library closure into the immutable System Image so the payload does
# not depend on the CI host after boot.
bundle_elf_deps() {
  local root="$1" pass=0
  while [ "$pass" -lt 8 ]; do
    pass=$((pass + 1)); local changed=0
    while IFS= read -r -d '' elf; do
      file "$elf" | grep -q 'ELF' || continue
      while IFS= read -r dep; do
        case "$dep" in
          /lib/*|/lib64/*|/usr/lib/*)
            [ -e "$dep" ] || continue
            local rel="${dep#/}" dst="$root/$rel"
            if [ ! -e "$dst" ]; then mkdir -p "$(dirname "$dst")"; cp -a "$dep" "$dst"; changed=1; fi
            ;;
        esac
      done < <(ldd "$elf" 2>/dev/null | awk '/=> \/(lib|usr\/lib)/ {print $3} /^\/(lib|usr\/lib)/ {print $1}')
    done < <(find "$root" -type f -perm -0100 -print0)
    [ "$changed" -eq 0 ] && break
  done
}

command -v file >/dev/null
command -v ldd >/dev/null
bundle_elf_deps "$ROOT"

[ -x "$ROOT/usr/bin/yazi" ] || { echo "Yazi binary missing" >&2; exit 1; }
[ -x "$ROOT/usr/bin/ya" ] || { echo "Yazi helper missing" >&2; exit 1; }
[ -x "$ROOT/usr/bin/luna-files" ] || { echo "Luna Files GUI missing" >&2; exit 1; }

echo "Built Yazi/Luna Files payload: $YAZI_TAG ($ACTUAL_YAZI_COMMIT)"
