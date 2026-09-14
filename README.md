# edotex

`edotex` is the Rust successor to `gotex`. It is a small command-line tool and
library wrapper around Tectonic for compiling LaTeX documents with a bundled
local TeX tree.

The tool embeds the project-local `texmf` directory, extracts it into a local
data directory on demand, and adds all extracted subdirectories as Tectonic
search paths. This makes local classes, packages, and fonts available during
compilation.

## Features

- Compile a `.tex` file to a sibling `.pdf`.
- Initialize an embedded local TeX tree.
- Provide the foundation for replacing the existing `gotex` workflow.
- Capture and stream Tectonic status output through any `std::io::Write`.
- Use additional TeX search paths from CLI or JSON config.
- Build and install through `make`.

## Usage

### Local static native dependencies (macOS and Linux)

With Rust, a C/C++ toolchain (Xcode on macOS), CMake, Make, curl, tar,
xz and shasum available, build
the native dependencies inside this checkout:

```sh
make deps
make cargo-check
make check
make build
```

`make deps` downloads pinned, SHA-256-checked source archives and builds pkgconf,
zlib, ICU, Graphite2, libpng and FreeType under `.deps/`. It does not install Homebrew
or write to system directories. The first build takes several minutes. Build
artifacts are ignored by Git. Linux also builds gperf, Expat and Fontconfig.
Native libraries are built statically; macOS SDK libraries and the Linux system
runtime (including libc and libstdc++) remain dynamic dependencies. HarfBuzz is
built by Tectonic itself. Linux uses the host's `/etc/fonts` configuration.
The Makefile automatically uses the local `pkg-config` paths when available.
For direct Cargo commands, use `sh scripts/with-native-deps.sh cargo ...`.
After moving the checkout, rebuild `.deps/` because installed metadata contains
absolute paths. The release workflow runs `make deps` before checks and builds
on both platforms, then checks the release binary's dynamic dependencies before
packaging. This is not a fully static musl build; Linux binaries still require
a system runtime compatible with the build runner.

The release matrix builds Linux x86_64, Linux ARM64 (`aarch64`, on
`ubuntu-24.04-arm`) and macOS ARM64 archives. Each binary is started with
`--version` before packaging. Windows releases are not configured yet.

Build a document:

```sh
cargo run -- path/to/document.tex
```

This writes `path/to/document.pdf`.

Use a custom local data directory:

```sh
cargo run -- --local-dir /tmp/edotex-data path/to/document.tex
```

Initialize the embedded TeX tree without compiling:

```sh
cargo run -- install
```

Add extra search paths:

```sh
cargo run -- \
  --tex-search-path path/to/local/styles \
  path/to/document.tex
```

## Configuration

`edotex` reads `./config.json` when it exists. CLI values override JSON values.

Example:

```json
{
  "server": {
    "port": 8080,
    "server_name": "localhost"
  },
  "tex": {
    "local_dir": "/home/user/.local/edotex",
    "tree_path": null,
    "cache_path": null,
    "search_paths": []
  }
}
```

Write a default config:

```sh
cargo run -- --write-config config.json
```

## Make Targets

```sh
make fmt          # format Rust code
make cargo-check  # cargo check --all-targets --locked
make check        # format check, compiler check, clippy, tests
make build        # release build
make install      # install target/release/edotex
make install-texmf
make doc          # build example documentation PDFs
make clean
```

The default install prefix is:

- Linux: `$HOME/.local`
- macOS: `/usr/local`

Override paths when needed:

```sh
make install INSTALL_PREFIX=/opt/edotex
make doc LOCAL_DIR=/tmp/edotex-data
```

## Library API

The main compiler API is:

```rust
use edotex::tex::{compile, TeX_Env};

let env = TeX_Env::default();
let input = std::fs::read("document.tex")?;
let mut out = std::io::stdout();

let result = compile(&env, input, &mut out, None)?;
std::fs::write("document.pdf", result.pdf)?;
```

To prepend TeX code before the document, pass it as the final argument:

```rust
let result = compile(
    &env,
    input,
    &mut out,
    Some(r"\newcommand{\usesolution}{nosolution}"),
)?;
```

Nonempty injected code is separated from the document by a newline. `None` or
an empty string leaves the input unchanged. From the shell, use `-i` or
`--inject` (single quotes preserve TeX backslashes and shell special characters):

```sh
edotex --inject '\newcommand{\usesolution}{nosolution}' document.tex
edotex -i '\def\usesolution{nosolution}' document.tex
```

Injection is a CLI-only configuration value and is not saved in JSON config files.

Initialize the embedded TeX tree manually:

```rust
edotex::tex::textree::init(&env)?;
```

## Documentation Examples

The bundled example documents live in:

```text
texmf/doc/latex/bflatex
```

They can be built with:

```sh
make doc
```

Current examples:

- `article.tex`
- `book.tex`
- `tikz.tex`

## Notes

Tectonic may download standard LaTeX resources into its own cache on the first
run. Set `XDG_CACHE_HOME` if the default cache location is not writable:

```sh
env XDG_CACHE_HOME=/tmp/edotex-cache cargo run -- path/to/document.tex
```

`Serve` mode is reserved in the CLI but is not implemented yet.

## License

MIT License, copyright (c) 2026 Benjamin Benno Falkner.
