#!/bin/sh
# Run Cargo or Make with the locally built native dependencies.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PREFIX="$ROOT/.deps/install"
if [ ! -x "$PREFIX/bin/pkg-config" ]; then
    echo 'Run make deps first.' >&2
    exit 1
fi
export PATH="$PREFIX/bin:$PATH"
export PKG_CONFIG="$PREFIX/bin/pkg-config"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export PKG_CONFIG_ALL_STATIC=1
exec "$@"
