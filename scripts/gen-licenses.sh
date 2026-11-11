#!/bin/sh
# Generate THIRD_PARTY_LICENSES.md: an overview and the license texts of
# everything edotex ships (Rust crates, native libraries, web editor, fonts).
# Usage: sh scripts/gen-licenses.sh [OUTPUT] [CARGO_FEATURES]
# Needs cargo-about (cargo install cargo-about --locked --features cli),
# the native sources from `make deps` and web/node_modules from `make web`.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT=${1:-$ROOT/THIRD_PARTY_LICENSES.md}
FEATURES=${2:-}
SRC="$ROOT/.deps/src"
LIC="$ROOT/licenses"

command -v cargo-about >/dev/null || {
    echo "cargo-about is missing: cargo install cargo-about --locked --features cli" >&2
    exit 1
}
test -d "$SRC" || { echo "Native sources missing: run make deps first." >&2; exit 1; }
test -d "$ROOT/web/node_modules" || { echo "web/node_modules missing: run make web first." >&2; exit 1; }

TMP=$(mktemp)
trap 'rm -f "$TMP" "$TMP.rust"' EXIT

# Append one license text as a fenced block; fall back to a link when the
# source is not available on this platform (e.g. Linux-only libraries on macOS).
license_block() {
    title=$1 file=$2 url=$3
    printf '### %s\n\n' "$title" >> "$TMP"
    if [ -f "$file" ]; then
        printf '```text\n' >> "$TMP"
        cat "$file" >> "$TMP"
        printf '\n```\n\n' >> "$TMP"
    else
        printf 'License text: <%s>\n\n' "$url" >> "$TMP"
    fi
}

cat "$LIC/overview.md" > "$TMP"
printf '\n' >> "$TMP"

# Rust crates
(cd "$ROOT" && cargo about generate --locked ${FEATURES:+--features "$FEATURES"} \
    "$LIC/rust.hbs" -o "$TMP.rust")
cat "$TMP.rust" >> "$TMP"

# Native libraries
HARFBUZZ=$(cd "$ROOT" && cargo metadata --format-version 1 --locked 2>/dev/null |
    python3 -c 'import json,sys,os
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == "tectonic_bridge_harfbuzz":
        print(os.path.join(os.path.dirname(p["manifest_path"]), "harfbuzz", "COPYING"))')
printf '## Native libraries\n\n' >> "$TMP"
license_block "HarfBuzz (Old MIT)" "$HARFBUZZ" https://github.com/harfbuzz/harfbuzz/blob/main/COPYING
license_block "ICU 77.1 (Unicode-3.0)" "$SRC/icu/LICENSE" https://github.com/unicode-org/icu/blob/release-77-1/LICENSE
license_block "zlib 1.3.1 (Zlib)" "$SRC/zlib-1.3.1/LICENSE" https://github.com/madler/zlib/blob/v1.3.1/LICENSE
license_block "libpng 1.6.55 (libpng-2.0)" "$SRC/libpng-1.6.55/LICENSE" https://github.com/pnggroup/libpng/blob/v1.6.55/LICENSE
license_block "FreeType 2.14.3 (FTL)" "$SRC/freetype-VER-2-14-3/docs/FTL.TXT" https://gitlab.freedesktop.org/freetype/freetype/-/blob/VER-2-14-3/docs/FTL.TXT
license_block "Graphite2 1.3.14 (MPL-2.0)" "$SRC/graphite-1.3.14/COPYING" https://github.com/silnrsi/graphite/blob/1.3.14/COPYING
printf 'Graphite2 is used under the Mozilla Public License 2.0; its text is listed under Rust crates.\n\n' >> "$TMP"
license_block "Expat 2.7.4 (MIT, Linux only)" "$SRC/expat-2.7.4/COPYING" https://github.com/libexpat/libexpat/blob/R_2_7_4/expat/COPYING
license_block "Fontconfig 2.16.0 (MIT-style, Linux only)" "$SRC/fontconfig-2.16.0/COPYING" https://gitlab.freedesktop.org/fontconfig/fontconfig/-/blob/2.16.0/COPYING

# Web editor: runtime packages bundled into web/build
printf '## Web editor\n\n' >> "$TMP"
for pkg in svelte @sveltejs/kit devalue esm-env clsx fflate; do
    dir="$ROOT/web/node_modules/$pkg"
    test -d "$dir" || continue
    version=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d["version"], d.get("license", "?"))' "$dir/package.json")
    file=$(find "$dir" -maxdepth 1 -iname 'license*' | head -1)
    license_block "$pkg $version" "$file" "https://www.npmjs.com/package/$pkg"
done

# Fonts
cat "$LIC/fonts.md" >> "$TMP"
printf '```text\n' >> "$TMP"
cat "$LIC/OFL-1.1.txt" >> "$TMP"
printf '```\n' >> "$TMP"

chmod 644 "$TMP"
mv "$TMP" "$OUT"
echo "created $OUT"
