#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${LUNA_NERD_FONTS_OUT:-${REPO_ROOT}/dist/nerd-fonts-root}"
SRC="${LUNA_DESKTOP_SRC:-${REPO_ROOT}/dist/sources}/nerd-fonts"
VERSION="${LUNA_NERD_FONTS_VERSION:-v3.5.1}"
JOBS="${LUNA_BUILD_JOBS:-$(nproc)}"

# Keep the base image useful for terminals and development without shipping the
# entire Nerd Fonts collection. SymbolsOnly provides the icon glyphs while the
# three monospaced families cover the common terminal/editor use cases.
FONTS=(
    "JetBrainsMono"
    "FiraCode"
    "Hack"
    "NerdFontsSymbolsOnly"
)

BASE_URL="https://github.com/ryanoasis/nerd-fonts/releases/download/${VERSION}"
CHECKSUMS="$SRC/SHA-256.txt"

rm -rf "$OUT"
mkdir -p "$OUT/usr/share/fonts/truetype" "$OUT/usr/share/licenses/luna-nerd-fonts" "$OUT/etc/luna" "$SRC"

curl -fsSL "${BASE_URL}/SHA-256.txt" -o "$CHECKSUMS"

for font in "${FONTS[@]}"; do
    archive="$SRC/${font}.tar.xz"
    curl -fsSL "${BASE_URL}/${font}.tar.xz" -o "$archive"

    expected="$(awk -v f="${font}.tar.xz" '$2 == f {print $1; exit}' "$CHECKSUMS")"
    [ -n "$expected" ] || {
        echo "missing checksum for ${font}.tar.xz" >&2
        exit 1
    }
    printf '%s  %s\n' "$expected" "$archive" | sha256sum -c -

    work="$SRC/${font}"
    rm -rf "$work"
    mkdir -p "$work"
    tar -xf "$archive" -C "$work"

    find "$work" -type f \( -iname '*.ttf' -o -iname '*.otf' \) -print0 |
        while IFS= read -r -d '' file; do
            install -Dm0644 "$file" "$OUT/usr/share/fonts/truetype/luna/$(basename "$file")"
        done

    find "$work" -type f \( -iname 'LICENSE*' -o -iname 'OFL.txt' -o -iname 'OFL.md' \) -print0 |
        while IFS= read -r -d '' file; do
            install -Dm0644 "$file" "$OUT/usr/share/licenses/luna-nerd-fonts/$font/$(basename "$file")"
        done

done

install -Dm0644 /dev/null "$OUT/usr/share/fonts/truetype/luna/.keep"
rm -f "$OUT/usr/share/fonts/truetype/luna/.keep"

cat > "$OUT/etc/luna/fonts.toml" <<EOF
[fonts]
provider = "nerd-fonts"
version = "${VERSION#v}"

[terminal]
family = "JetBrainsMono Nerd Font Mono"

[editor]
family = "JetBrainsMono Nerd Font"

[symbols]
family = "Symbols Nerd Font Mono"

[packages]
jetbrains_mono = "JetBrainsMono"
fira_code = "FiraCode"
hack = "Hack"
symbols_only = "NerdFontsSymbolsOnly"
EOF

# Provide a stable fontconfig alias without forcing every GUI application to use
# the developer font. Applications can continue to choose their own family.
mkdir -p "$OUT/etc/fonts/conf.d"
cat > "$OUT/etc/fonts/conf.d/99-luna-nerd-fonts.conf" <<'EOF'
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <alias binding="strong">
    <family>JetBrains Mono</family>
    <prefer><family>JetBrainsMono Nerd Font</family></prefer>
  </alias>
</fontconfig>
EOF

cat > "$OUT/etc/luna/nerd-fonts.sha256" <<EOF
# Nerd Fonts ${VERSION}
$(for font in "${FONTS[@]}"; do awk -v f="${font}.tar.xz" '$2 == f {print $0; exit}' "$CHECKSUMS"; done)
EOF

count="$(find "$OUT/usr/share/fonts/truetype/luna" -type f \( -iname '*.ttf' -o -iname '*.otf' \) | wc -l)"
[ "$count" -gt 0 ] || { echo "no font files installed" >&2; exit 1; }

echo "Installed $count Nerd Font files into $OUT"
printf 'Nerd Fonts %s payload ready (%s files).\n' "$VERSION" "$count"
