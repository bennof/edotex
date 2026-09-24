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

.PHONY: all clean check cargo-check fmt run build install install-texmf doc clean-doc commit-version version push

all: check build

.PHONY: deps
deps:
	sh scripts/build-native-deps.sh

check:
	cargo fmt --check
	cargo check --all-targets --locked
	cargo clippy -- -D warnings
	cargo test

cargo-check:
	cargo check --all-targets --locked

fmt:
	cargo fmt --all

run:
	cargo run --

build:
	cargo build --release

install: build
	@echo "Installing $(BINARY_NAME) on $(SYSTEM_NAME) to $(INSTALL_DIR)"
	install -d "$(INSTALL_DIR)"
	install -m 755 "target/release/$(BINARY_NAME)" "$(INSTALL_DIR)/$(BINARY_NAME)"

install-texmf:
	cargo run -- install --local-dir "$(LOCAL_DIR)"

update-texmf:
	rsync -av  --exclude=/tex/latex/bflatex/fontconfig.tex ./texmf/ "$(LOCAL_DIR)/texmf/"



doc: $(DOC_PDF)

$(DOC_DIR)/%.pdf: $(DOC_DIR)/%.tex
	cargo run -- --local-dir "$(LOCAL_DIR)" "$<"

clean-doc:
	rm -f $(DOC_PDF)

clean:
	cargo clean
	rm -rf "$(LOCAL_DIR)"
	rm -f $(DOC_PDF)

commit-version:
	git add .
	git commit -m "Version $(VERSION)"
	git tag -a "v$(VERSION)"

version:
	git describe --tags --exact-match

push:
	git push
	git push --tags
