#!/bin/sh
# Build Tectonic's native dependencies locally, without a package manager.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEPS="$ROOT/.deps"
PREFIX="$DEPS/install"
JOBS=${JOBS:-4}
SYSTEM_NAME=$(uname -s)
case "$SYSTEM_NAME" in Darwin|Linux) ;; *) echo "Unsupported OS: $SYSTEM_NAME" >&2; exit 1 ;; esac
mkdir -p "$DEPS/downloads" "$DEPS/src" "$DEPS/build" "$PREFIX"
export PATH="$PREFIX/bin:$PATH"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
# Linux executables use PIE; static archives must contain relocatable code.
export CFLAGS="${CFLAGS:-} -fPIC"
export CXXFLAGS="${CXXFLAGS:-} -fPIC"

fetch() {
    name=$1 url=$2 checksum=$3
    archive="$DEPS/downloads/$name"
    if [ ! -f "$archive" ]; then
        curl -fLsS --retry 3 "$url" -o "$archive.tmp"
        mv "$archive.tmp" "$archive"
    fi
    printf '%s  %s\n' "$checksum" "$archive" | shasum -a 256 -c -
    tar -xf "$archive" -C "$DEPS/src"
}

fetch pkgconf.tar.xz https://distfiles.ariadne.space/pkgconf/pkgconf-2.3.0.tar.xz 3a9080ac51d03615e7c1910a0a2a8df08424892b5f13b0628a204d3fcce0ea8b
fetch zlib.tar.gz https://zlib.net/fossils/zlib-1.3.1.tar.gz 9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23
fetch icu.tgz https://github.com/unicode-org/icu/releases/download/release-77-1/icu4c-77_1-src.tgz 588e431f77327c39031ffbb8843c0e3bc122c211374485fa87dc5f3faff24061
fetch graphite.tar.gz https://github.com/silnrsi/graphite/archive/refs/tags/1.3.14.tar.gz 7a3b342c5681921ce2e0c2496509d30b5b078399d5a7bd2358f95166d57d91df
fetch png.tar.gz https://github.com/pnggroup/libpng/archive/refs/tags/v1.6.55.tar.gz 71a2c5b1218f60c4c6d2f1954c7eb20132156cae90bdb90b566c24db002782a6
fetch freetype.tar.gz https://github.com/freetype/freetype/archive/refs/tags/VER-2-14-3.tar.gz dc49de6b01a266eef4876a4dd34d9842c475d3e28ff2eff63bd2fb760ab56261
if [ "$SYSTEM_NAME" = Linux ]; then
    fetch gperf.tar.gz https://ftp.gnu.org/gnu/gperf/gperf-3.1.tar.gz 588546b945bba4b70b6a3a616e80b4ab466e3f33024a352fc2198112cdbb3ae2
    fetch expat.tar.xz https://github.com/libexpat/libexpat/releases/download/R_2_7_4/expat-2.7.4.tar.xz 9e9cabb457c1e09de91db2706d8365645792638eb3be1f94dbb2149301086ac0
    fetch fontconfig.tar.xz https://www.freedesktop.org/software/fontconfig/release/fontconfig-2.16.0.tar.xz 6a33dc555cc9ba8b10caf7695878ef134eeb36d0af366041f639b1da9b6ed220
fi

mkdir -p "$DEPS/build/pkgconf" "$DEPS/build/icu"
if [ ! -x "$PREFIX/bin/pkgconf" ]; then (
    cd "$DEPS/build/pkgconf"
    "$DEPS/src/pkgconf-2.3.0/configure" --prefix="$PREFIX" --disable-shared --enable-static
    make -j "$JOBS"
    make install
); fi
ln -sf pkgconf "$PREFIX/bin/pkg-config"
if [ ! -f "$PREFIX/lib/libz.a" ]; then (
    cd "$DEPS/src/zlib-1.3.1"
    ./configure --prefix="$PREFIX" --static
    make -j "$JOBS"
    make install
); fi
if [ ! -f "$PREFIX/lib/libicuuc.a" ]; then (
    cd "$DEPS/build/icu"
    "$DEPS/src/icu/source/configure" --prefix="$PREFIX" --disable-shared --enable-static --disable-tests --disable-samples
    make -j "$JOBS"
    make install
); fi

build_cmake() {
    name=$1 source=$2
    shift 2
    # Generated headers can retain the previous system-library configuration.
    rm -rf "$DEPS/build/$name"
    cmake -S "$DEPS/src/$source" -B "$DEPS/build/$name" \
        -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX" \
        -DCMAKE_INSTALL_LIBDIR=lib -DCMAKE_PREFIX_PATH="$PREFIX" \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
        -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DBUILD_SHARED_LIBS=OFF "$@"
    cmake --build "$DEPS/build/$name" --parallel "$JOBS"
    cmake --install "$DEPS/build/$name"
}
# Graphite 1.3.14 uses a shared-library-only expression in a macOS test.
# TARGET_FILE works for both static and shared libraries.
sed 's/TARGET_SONAME_FILE:graphite2/TARGET_FILE:graphite2/g' \
    "$DEPS/src/graphite-1.3.14/src/CMakeLists.txt" > "$DEPS/build/graphite-CMakeLists.txt"
cp "$DEPS/build/graphite-CMakeLists.txt" "$DEPS/src/graphite-1.3.14/src/CMakeLists.txt"
build_cmake graphite graphite-1.3.14
build_cmake png libpng-1.6.55 -DPNG_SHARED=OFF -DPNG_FRAMEWORK=OFF -DPNG_TESTS=OFF -DPNG_TOOLS=OFF \
    -DZLIB_LIBRARY="$PREFIX/lib/libz.a" -DZLIB_INCLUDE_DIR="$PREFIX/include"
build_cmake freetype freetype-VER-2-14-3 -DFT_DISABLE_HARFBUZZ=ON -DFT_DISABLE_BROTLI=ON -DFT_DISABLE_BZIP2=ON \
    -DZLIB_LIBRARY="$PREFIX/lib/libz.a" -DZLIB_INCLUDE_DIR="$PREFIX/include" \
    -DPNG_LIBRARY="$PREFIX/lib/libpng16.a" -DPNG_PNG_INCLUDE_DIR="$PREFIX/include"
if [ "$SYSTEM_NAME" = Linux ]; then
    (
        cd "$DEPS/src/gperf-3.1"
        ./configure --prefix="$PREFIX"
        make -j "$JOBS"
        make install
    )
    build_cmake expat expat-2.7.4 -DEXPAT_SHARED_LIBS=OFF -DEXPAT_BUILD_TESTS=OFF \
        -DEXPAT_BUILD_EXAMPLES=OFF -DEXPAT_BUILD_TOOLS=OFF -DEXPAT_BUILD_DOCS=OFF
    mkdir -p "$DEPS/build/fontconfig"
    (
        cd "$DEPS/build/fontconfig"
        export PKG_CONFIG_ALLOW_SYSTEM_LIBS=1
        export FREETYPE_LIBS="$(pkg-config --static --libs freetype2)"
        export EXPAT_LIBS="$(pkg-config --static --libs expat)"
        "$DEPS/src/fontconfig-2.16.0/configure" --prefix="$PREFIX" \
            --disable-shared --enable-static --disable-docs --disable-nls \
            --disable-cache-build --sysconfdir=/etc --localstatedir=/var \
            --with-default-fonts=/usr/share/fonts --with-cache-dir=/var/cache/fontconfig
        make -j "$JOBS"
        # Install only the development library; retain the host's font configuration.
        make -C src install-libLTLIBRARIES
        make -C fontconfig install-fontconfigincludeHEADERS
        make install-pkgconfigDATA
    )
    pkg-config --modversion expat fontconfig
fi
pkg-config --modversion zlib icu-uc graphite2 libpng freetype2
