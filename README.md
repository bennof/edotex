# edotex

`edotex` is the Rust successor to `gotex`: a command-line tool and library for
compiling LaTeX documents with Tectonic and an embedded TeX tree.

Current version: **0.1.0-rc.1** (release candidate).

The binary includes the classes, packages and fonts from `texmf/`. It extracts
that tree into a local data directory and uses its subdirectories as additional
TeX search paths. Standard LaTeX resources are supplied by Tectonic and may be
downloaded on first use.

## Binary

`edotex` compiles TeX documents locally, installs the TeX resources and serves
the HTTP API with the embedded editor:

```sh
edotex document.tex
edotex serve --host 127.0.0.1 --port 8080
```

It defaults to build mode. `serve` exposes `POST /api/tex` and serves the
[editor](web/) from `web/build/` for all other paths; unknown `/api` paths
return 404. The entry point is `src/bin/edotex.rs`.

The editor is a Git submodule in `web/` and is embedded at compile time, so
building from source needs Node.js 22.18 or newer. `make web` builds it, and
`make check` and `make build` run it first. To build without make:

```sh
make web
sh scripts/with-native-deps.sh cargo build --release
```

The `server` Cargo feature is enabled by default and required by the binary.
Library users can disable it with `default-features = false` to drop the server
modules, `tex::handle_tex` and their direct dependencies. Compiler dependencies
may themselves use networking libraries.

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
tar, xz, `shasum`, Git and Node.js 22.18 or newer with npm for the editor.

```sh
make build
./target/release/edotex --version
```

`make build` runs `make check` first, which builds the editor and the native
dependencies before checking and testing.

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
| `make web` | Build the editor into `web/build/` (checks out the submodule if missing). |
| `make fmt` | Format Rust code. |
| `make cargo-check` | Run `cargo check` for all targets and for the library without default features. |
| `make check` | Run `make web` and `make deps`, then check formatting, compile, run Clippy with warnings denied and run tests. |
| `make build` | Run `make check`, then build `target/release/edotex`. |
| `make run` | Run the debug binary in build mode. |
| `make serve` | Start `edotex serve` from the debug build. |
| `make install` | Install `target/release/edotex`; run `make build` first. |
| `make doc` | Build documentation PDFs that are missing or older than their `.tex` source. |
| `make clean` | Remove Cargo output, `LOCAL_DIR`, generated documentation PDFs and editor build output. |

Plain `make` runs `make build`. `make clean` retains `.deps/` but deletes the
configured `LOCAL_DIR`. Initialize the TeX tree with `edotex install`.

The default binary installation prefix is `$HOME/.local` on Linux and
`/usr/local` on macOS. Override it as needed:

```sh
make build
make install INSTALL_PREFIX="$HOME/.local"
make doc LOCAL_DIR=/tmp/edotex-data
```

The documentation sources are `article.tex`, `book.tex` and `tikz.tex` in
[`texmf/doc/latex/bflatex`](texmf/doc/latex/bflatex). To rebuild all examples after
changing a package, delete the PDFs in that directory and run `make doc`.

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
    textree::init(&env, None)?;

    let input = std::fs::read("document.tex")?;
    let mut out = std::io::stdout();
    let result = compile(
        &env,
        input,
        &mut out,
        Some(r"\newcommand{\usesolution}{nosolution}"),
        None,
        false,
    )?;
    std::fs::write("document.pdf", result.pdf)?;
    Ok(())
}
```

Pass `None` as `inject` to compile without injection. The last argument is the
directory searched first for relative file names such as `\input{chapter}`;
`None` uses the current working directory. With the final `tex_log` flag set,
the complete TeX log of the last pass (`texput.log`) is written to the output
and `output_log` after the run; it contains LaTeX and package warnings such as
undefined references, which Tectonic does not report as status messages. `TeX_Output`
contains `pdf` bytes and `output_log`. On failure, `TeX_Error` contains a message
and the captured log. Tectonic status messages are also written to the supplied
`std::io::Write` destination.

### Embedded HTTP files

`server::handle_embedded` serves a `rust-embed` asset type through Axum:

```rust,no_run
use axum::Router;
use edotex::server::{Server, handle_embedded};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "public/"]
struct Assets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .fallback(|request| handle_embedded(request, Assets, Some("index.html")));
    let mut server = Server::new("127.0.0.1:8080".into());
    server.set_router(router);
    server.listen().await
}
```

Both `handle_static(request, base_dir, fallback)` and
`handle_embedded(request, Assets, fallback)` return `Result<Response<Body>, StatusCode>` and share request validation, response
headers and error responses. Disk I/O errors other than missing files are logged
and produce HTTP 500. HEAD error responses also have no body.

Create `public/` and its files before compiling. The `debug-embed` feature is
enabled, so assets are embedded in both debug and release builds; rebuild after
changing them. GET serves file bytes, HEAD returns headers without a body, and
other methods receive 405. `/` and directory paths resolve to `index.html`;
missing assets return 404. With `fallback`, e.g. `Some("index.html")` for a
single-page app, missing paths without a file extension (`/doc/42`) serve that
file with 200; missing paths with an extension (`/app.js`) still return 404.
Pass `None` to disable it. Responses include MIME type, length and, when
available, the modification time. Parent path components are rejected. As with
the disk handler, URL percent escapes are not decoded; conditional caching and
Range requests are not implemented. Register this handler explicitly to serve
files; `edotex serve` uses it with `Some("index.html")` for the editor.

### Streaming TeX HTTP handler

`tex::handle_tex(request, env, max_body_size, compile_slots)` accepts POST
requests in one of two forms:

- A raw body containing a complete TeX document.
- `multipart/form-data`: the first part is the TeX document; every further part
  is a supporting file stored under its relative `filename`, for example
  `images/logo.png`. Missing, empty, absolute, `..` and duplicate file names
  return 400.

The TeX document stays in memory. Supporting files are written to a temporary
directory, which is the compiler's root for relative file names and is removed
after compilation. Single documents get an empty directory, so the server's
working directory is not searched.

```sh
curl -F "tex=@main.tex" -F "file=@logo.png;filename=images/logo.png" \
  http://127.0.0.1:8080/api/tex
```

`max_body_size` limits the whole body, including all parts; `None` accepts any
size. Axum's `DefaultBodyLimit` does not apply. `compile_slots` is a shared
semaphore limiting concurrent compilations; `None` means unlimited. Initialize
the TeX tree once before serving:

```rust,no_run
use std::sync::Arc;

use axum::{Router, routing::post};
use edotex::{server::Server, tex::{TeX_Env, handle_tex, textree}};
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = TeX_Env::default();
    textree::init(&env, None)?;
    let slots = Arc::new(Semaphore::new(4));
    let router = Router::new().route("/compile", post(move |request| {
        handle_tex(request, env.clone(), Some(20 * 1024 * 1024), Some(slots.clone()))
    }));
    let mut server = Server::new("127.0.0.1:8080".into());
    server.set_router(router);
    server.listen().await
}
```

The response is `multipart/mixed` with a per-response boundary:

1. An `application/json` part: `{"type":"progress","status":"processing"}`.
2. An `application/x-ndjson` part containing compiler lines as
   `{"type":"log","message":"..."}` records, streamed as they become available.
3. On success, an `application/pdf` part with
   `Content-Disposition: attachment; filename="result.pdf"` and binary PDF bytes.
   On failure, an `application/json` part with `type: "error"`, `status: "failed"`
   and `message`, without a PDF part.

Each response ends with the closing multipart boundary. HTTP transport chunks
are not part boundaries: clients must parse the multipart body. Compilation
failures after streaming begins retain HTTP 200 and are reported in the JSON
part. Before streaming, an empty/unreadable body or invalid upload returns 400,
an oversized body 413, no free compile slot 503, and unsupported methods 405
with `Allow: POST`.

Compilation runs in `spawn_blocking`; a bounded channel applies backpressure to
log output. Tectonic status messages, including box warnings and errors, arrive
during the run; the engine stdout and the complete TeX log of the last pass
(`texput.log`, with LaTeX and package warnings) follow at the end. Disconnecting a client closes the
stream but does not cancel an already-running synchronous compilation. This
handler uses the existing compiler configuration and adds no compilation timeout
or sandbox for untrusted TeX. `edotex serve` exposes this handler at `POST /api/tex`; the example above
uses `/compile` as a custom route.

## License

[MIT License](LICENSE), copyright (c) 2026 Benjamin Benno Falkner.

## Note on AI-assisted development

This project is developed with the assistance of AI tools.
