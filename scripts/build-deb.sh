#!/bin/sh
# Package the static Linux binary as a .deb for Debian and Ubuntu.
# Usage: sh scripts/build-deb.sh [BINARY] [DEB]
#   BINARY defaults to target/release/edotex, DEB to dist/edotex.deb.
# The architecture is taken from the binary (amd64 or arm64). Needs dpkg-deb.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BINARY=${1:-$ROOT/target/release/edotex}
DEB=${2:-$ROOT/dist/edotex.deb}
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
# Debian sorts "~" before the release: 0.2.0~beta.2 < 0.2.0.
DEB_VERSION=$(printf '%s' "$VERSION" | sed 's/-/~/')

case "$(file -b "$BINARY")" in
    *x86-64*) ARCH=amd64 ;;
    *aarch64*|*"ARM aarch64"*) ARCH=arm64 ;;
    *) echo "Unsupported binary architecture: $(file -b "$BINARY")" >&2; exit 1 ;;
esac

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
PKG="$STAGE/edotex"
DOC="$PKG/usr/share/doc/edotex"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$DOC"
install -m 755 "$BINARY" "$PKG/usr/bin/edotex"

# License files: Debian expects /usr/share/doc/<package>/copyright.
{
    printf 'Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n'
    printf 'Upstream-Name: edotex\n'
    printf 'Source: https://github.com/bennof/edotex\n'
    printf 'Comment: Third-party software and fonts shipped with edotex are listed\n'
    printf ' with their licenses in /usr/share/doc/edotex/THIRD_PARTY_LICENSES.md.\n\n'
    printf 'Files: *\n'
    printf 'Copyright: 2026 Benjamin Benno Falkner\n'
    printf 'License: MIT\n'
    # Continuation lines of the license text: leading space, "." for empty lines.
    sed -e 's/^$/./' -e 's/^/ /' "$ROOT/LICENSE"
} > "$DOC/copyright"
install -m 644 "$ROOT/LICENSE" "$DOC/LICENSE"
install -m 644 "$ROOT/THIRD_PARTY_LICENSES.md" "$DOC/THIRD_PARTY_LICENSES.md"

SIZE_KB=$(du -sk "$PKG/usr" | cut -f1)
cat > "$PKG/DEBIAN/control" <<EOF
Package: edotex
Version: $DEB_VERSION
Architecture: $ARCH
Maintainer: Benjamin Benno Falkner <benno.falkner@gmail.com>
Installed-Size: $SIZE_KB
Recommends: ca-certificates, fontconfig-config
Section: tex
Priority: optional
Homepage: https://github.com/bennof/edotex
Description: LaTeX compiler with embedded TeX tree, based on Tectonic
 edotex compiles LaTeX documents with Tectonic and ships its own classes,
 packages and fonts. It also serves a browser-based editor. The binary is
 statically linked; standard LaTeX resources are downloaded over HTTPS on
 first use.
EOF

mkdir -p "$(dirname "$DEB")"
dpkg-deb --build --root-owner-group "$PKG" "$DEB"
dpkg-deb --info "$DEB"
dpkg-deb --contents "$DEB"
