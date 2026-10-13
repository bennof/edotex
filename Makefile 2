# Copyright (c) 2026 Benjamin Benno Falkner
# SPDX-License-Identifier: MIT

SYSTEM_NAME := $(shell uname -s)
BINARY_NAME := edotex

# Prefer the project-local native libraries when make deps has installed them.
NATIVE_PREFIX := $(CURDIR)/.deps/install
ifneq ($(wildcard $(NATIVE_PREFIX)/bin/pkg-config),)
export PATH := $(NATIVE_PREFIX)/bin:$(PATH)
export PKG_CONFIG := $(NATIVE_PREFIX)/bin/pkg-config
export PKG_CONFIG_PATH := $(NATIVE_PREFIX)/lib/pkgconfig$(if $(PKG_CONFIG_PATH),:$(PKG_CONFIG_PATH))
export PKG_CONFIG_ALL_STATIC := 1
endif

ifeq ($(SYSTEM_NAME),Darwin)
INSTALL_PREFIX ?= /usr/local
else ifeq ($(SYSTEM_NAME),Linux)
INSTALL_PREFIX ?= $(HOME)/.local
else
$(error Unsupported operating system: $(SYSTEM_NAME))
endif

INSTALL_DIR ?= $(INSTALL_PREFIX)/bin
LOCAL_DIR ?= $(HOME)/.local/$(BINARY_NAME)
DOC_DIR ?= texmf/doc/latex/bflatex

VERSION := $(shell cargo pkgid | cut -d\# -f2)
DOC_TEX := $(wildcard $(DOC_DIR)/*.tex)
DOC_PDF := $(DOC_TEX:.tex=.pdf)

.PHONY: all clean check cargo-check fmt run build web install install-texmf doc clean-doc commit-version version push

all:  build

.PHONY: deps
deps:
	sh scripts/build-native-deps.sh

check: web deps
	cargo fmt --check
	cargo check --all-targets --locked
	cargo check --lib --no-default-features --locked
	cargo clippy --all-targets -- -D warnings
	cargo clippy --lib --no-default-features -- -D warnings
	cargo test --locked

cargo-check: 
	cargo check --all-targets --locked
	cargo check --lib --no-default-features --locked

fmt:
	cargo fmt --all

run:
	cargo run --

serve:
	sh scripts/with-native-deps.sh cargo run -- serve

build: check  web
	cargo build --release --locked

# Build the editor into web/build/, which edotex embeds at compile time.
# The submodule is only checked out when missing, so local work in web/ is kept.
web:
	test -f web/package.json || git submodule update --init web
	cd web && npm ci && npm run build

install: 
	@echo "Installing $(BINARY_NAME) on $(SYSTEM_NAME) to $(INSTALL_DIR)"
	install -d "$(INSTALL_DIR)"
	install -m 755 target/release/edotex "$(INSTALL_DIR)/"

doc: $(DOC_PDF)

$(DOC_DIR)/%.pdf: $(DOC_DIR)/%.tex
	cargo run -- --local-dir "$(LOCAL_DIR)" "$<"


clean:
	cargo clean
	rm -rf "$(LOCAL_DIR)"
	rm -f $(DOC_PDF)
	rm -rf web/build web/.svelte-kit web/node_modules

commit-version:
	#cargo check --all-targets
	#git add .
	#git commit -m "Version $(VERSION)"
	git tag -a "v$(VERSION)"
	git push
	git push --tags

version:
	git describe --tags --exact-match
