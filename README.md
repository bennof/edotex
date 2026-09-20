# edotex

`edotex` is the Rust successor to `gotex`: a command-line tool and library for
compiling LaTeX documents with Tectonic and an embedded TeX tree.

Current version: **0.1.0-rc.1** (release candidate).

The binary includes the classes, packages and fonts from `texmf/`. It extracts
that tree into a local data directory and uses its subdirectories as additional
TeX search paths. Standard LaTeX resources are supplied by Tectonic and may be
downloaded on first use.

## Quick start

With the `edotex` binary on your `PATH`:

```sh
edotex document.tex
edotex --version
edotex --help
```

Compiling `document.tex` writes `document.pdf` next to the input file. The
explicit form `edotex build document.tex` works as well. An existing output PDF
is overwritten.

Use a custom data directory or initialize the embedded tree without compiling:

```sh
edotex --local-dir /tmp/edotex-data document.tex
edotex install --local-dir /tmp/edotex-data
```

On Linux and macOS, the default data directory is `$HOME/.local/edotex`.
Initialization preserves an existing nonempty TeX tree; it does not refresh
that tree automatically when the binary is updated. To try an updated embedded
tree, use a fresh local data directory.

## Injecting TeX code

Use `-i` or `--inject` to prepend TeX code before the document, including before
`\documentclass`:

```sh
edotex -i '\def\usesolution{nosolution}' document.tex
edotex --inject '\newcommand{\usesolution}{nosolution}' document.tex
```

In the document, provide a default that preserves an injected definition:

```latex
\documentclass{edoxarticle}
\usepackage{edoxworksheet}
\providecommand{\usesolution}{solution}

\begin{document}
\begin{worksheet}[\usesolution]{Example}
  \exercise{What is $1+1$? \solve{$2$}}
\end{worksheet}
\end{document}
```

The optional mode is expanded at the start of `worksheet`, `test` and `exam`.
Without injection, this example uses `solution`; with the commands above it
uses `nosolution`.

Nonempty injected code is followed by a newline before the original input.
Omitting the option or passing an empty string leaves the input unchanged.
Use single quotes in a POSIX shell to preserve TeX backslashes and shell special
characters. `\newcommand` requires an undefined command; `\def` can overwrite
an existing definition. Injection is passed per invocation and is not stored
in the JSON configuration.

## TeX paths and configuration

`edotex` reads `./config.json` if it exists. Use `-c` or `--config` for another
path. Configuration values override defaults; explicit CLI values override
configuration values. A TeX-only configuration can contain:

```json
{
  "tex": {
    "local_dir": "/home/user/.local/edotex",
    "tree_path": null,
    "cache_path": null,
    "search_paths": []
  }
}
```

| Option | Purpose |
| --- | --- |
| `-c`, `--config PATH` | Read configuration from the given path if it exists. |
| `--local-dir PATH` | Base directory for the extracted tree and format cache. |
| `--texmf PATH` | Override the TeX tree directory. |
| `--tectonic-cache PATH` | Override Tectonic's format cache directory. |
| `--tex-search-path PATH` | Add an input search path; may be repeated. |
| `-i`, `--inject CODE` | Prepend TeX code to the document. |

Without overrides, the tree is under `<local-dir>/texmf` and the format cache
under `<local-dir>/tectonic-cache`. CLI search paths replace the list from JSON.

```sh
edotex --tex-search-path ./styles --tex-search-path ./images document.tex
```

The compiler receives the document as a memory buffer. Relative auxiliary
inputs are resolved from the working directory and configured search paths.
Run from the document's directory or add the required search paths explicitly.

Tectonic also maintains a separate cache for downloaded bundle resources. On
macOS this is under `~/Library/Caches/TectonicProject.Tectonic`; changing
`--tectonic-cache` only changes the format cache. The bundle cache needs to be
writable, and initial compilation may require network access.

## Building from source

Development is supported on Linux and macOS. Prerequisites are Rust with
rustfmt and Clippy, a C/C++ toolchain (Xcode tools on macOS), CMake, Make, curl,
tar, xz and `shasum`.

```sh
make deps
make check
make build
./target/release/edotex --version
```

`make deps` builds pinned, SHA-256-checked source archives locally under `.deps/`.
It does not install Homebrew or install dependencies into system directories.
The first build takes several minutes.

- Both platforms: pkgconf, zlib, ICU, Graphite2, libpng and FreeType.
- Linux additionally: gperf, Expat and Fontconfig.
- Tectonic builds its bundled HarfBuzz through Cargo.

The Makefile automatically uses the local libraries once installed. For direct
Cargo commands, use the wrapper:

```sh
sh scripts/with-native-deps.sh cargo check --all-targets --locked
sh scripts/with-native-deps.sh cargo run -- -i '\def\usesolution{nosolution}' document.tex
```

`.deps/` is ignored by Git. Its generated metadata contains absolute paths;
after moving the checkout, remove the old `.deps/` directory and run `make deps`
again. Keep `Cargo.lock` versioned and consistent with `Cargo.toml`.

### Make targets

| Target | Action |
| --- | --- |
| `make deps` | Build local native dependencies. |
| `make fmt` | Format Rust code. |
| `make cargo-check` | Run `cargo check --all-targets --locked`. |
| `make check` | Check formatting, compile, run Clippy with warnings denied, and run tests. |
| `make build` | Build `target/release/edotex`. |
| `make install` | Build and install the binary. |
| `make install-texmf` | Initialize the embedded TeX tree. |
| `make doc` | Build documentation PDFs that are missing or older than their `.tex` source. |
| `make clean-doc` | Remove generated documentation PDFs. |
| `make clean` | Remove Cargo output, `LOCAL_DIR` and generated documentation PDFs. |

Plain `make` runs checks and the release build; run `make deps` first on a fresh
checkout. `make clean` retains `.deps/` but deletes the configured `LOCAL_DIR`.

The default binary installation prefix is `$HOME/.local` on Linux and
`/usr/local` on macOS. Override it as needed:

```sh
make install INSTALL_PREFIX="$HOME/.local"
make doc LOCAL_DIR=/tmp/edotex-data
```

The documentation sources are `article.tex`, `book.tex` and `tikz.tex` in
[`texmf/doc/latex/bflatex`](texmf/doc/latex/bflatex). To rebuild all examples after
changing a package, run `make clean-doc` followed by `make doc`.

## Releases

The [GitHub workflow](.github/workflows/release.yml) checks changes on `main`,
pull requests and `v*` tags, and can also be started manually. It defines these
release archives:

| Platform | Archive |
| --- | --- |
| Linux x86_64 | `edotex-linux-x86_64.tar.gz` |
| Linux ARM64 | `edotex-linux-aarch64.tar.gz` |
| macOS ARM64 (Apple Silicon) | `edotex-macos-aarch64.tar.gz` |

Each archive contains the `edotex` executable. The workflow builds local native
dependencies, checks dynamic linkage and runs `edotex --version` before
packaging. Tags such as `v0.1.0-rc.1` trigger publication to GitHub Releases.
Windows and Intel macOS archives are not configured.

Third-party native libraries are linked statically. Operating-system libraries
remain dynamic dependencies: macOS system frameworks and the Linux runtime,
including libc and libstdc++. Linux also uses the host's `/etc/fonts`
configuration. These are not fully static musl binaries; use a system runtime
compatible with the build runner. The initial TeX resource downloads are still
needed even when using a release binary.

## Library API

Initialize the embedded tree before compiling with it:

```rust
use edotex::tex::{compile, textree, TeX_Env};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = TeX_Env::default();
    textree::init(&env)?;

    let input = std::fs::read("document.tex")?;
    let mut out = std::io::stdout();
    let result = compile(
        &env,
        input,
        &mut out,
        Some(r"\newcommand{\usesolution}{nosolution}"),
    )?;
    std::fs::write("document.pdf", result.pdf)?;
    Ok(())
}
```

Pass `None` as the final argument to compile without injection. `TeX_Output`
contains `pdf` bytes and `output_log`. On failure, `TeX_Error` contains a message
and the captured log. Tectonic status messages are also written to the supplied
`std::io::Write` destination.

## License

[MIT License](LICENSE), copyright (c) 2026 Benjamin Benno Falkner.

## Note on AI-assisted development

This project is developed with the assistance of AI tools.
