#!/bin/sh
# Build a fully static Linux release binary against musl inside an Alpine container.
# Usage: sh scripts/build-linux-musl.sh   (needs Docker; builds for the host architecture)
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
IMAGE=${RUST_ALPINE_IMAGE:-rust:1-alpine}

docker run --rm \
    -v "$ROOT:/src" -w /src \
    -v "${CARGO_HOME:-$HOME/.cargo}/registry:/usr/local/cargo/registry" \
    -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
    -e JOBS="${JOBS:-4}" \
    "$IMAGE" sh -euc '
        apk add --no-cache build-base cmake curl perl perl-utils python3 \
            linux-headers git nodejs npm
        # Files written by root must stay usable for later steps on the host.
        trap "chown -R \$HOST_UID:\$HOST_GID /src/.deps /src/target /src/web 2>/dev/null || true" EXIT
        git config --global --add safe.directory "*"
        sh scripts/build-native-deps.sh
        make web
        sh scripts/with-native-deps.sh cargo build --release --locked
        ./target/release/edotex --version
    '
