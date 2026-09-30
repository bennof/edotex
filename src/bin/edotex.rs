use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Router,
    http::StatusCode,
    routing::{any, post},
};
use edotex::{
    config::{Config, Mode},
    server::{Server, handle_embedded},
    tex,
};
use rust_embed::Embed;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

/// Maximum size of a TeX request body (20 MiB).
const MAX_BODY_SIZE: usize = 20 * 1024 * 1024;
/// Maximum number of TeX compilations running at the same time.
const MAX_CONCURRENT_COMPILES: usize = 4;

/// The editor web app, built into `web/build/` before compiling.
#[derive(Embed)]
#[folder = "web/build/"]
struct Web;

/// Compiles `cfg.input` and writes the PDF to `cfg.output` or next to the input.
fn build_mode(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    tex::textree::init(&cfg.tex, None)?;

    let input_path = cfg
        .input
        .as_deref()
        .ok_or("build mode needs an input .tex file")?;
    let input = fs::read(input_path)?;

    let mut stdout = io::stdout();
    let out = match tex::compile(
        &cfg.tex,
        input,
        &mut stdout,
        cfg.inject.as_deref(),
        None,
        false,
    ) {
        Ok(out) => out,
        Err(err) => {
            return Err(Box::new(err));
        }
    };

    let output_path = match &cfg.output {
        Some(path) => path.clone(),
        None => pdf_output_path(input_path),
    };
    println!("write: {}", output_path.display());
    fs::write(output_path, out.pdf)?;

    Ok(())
}

fn pdf_output_path(input_path: &Path) -> PathBuf {
    let mut output_path = input_path.to_path_buf();
    output_path.set_extension("pdf");
    output_path
}

#[tokio::main]
async fn serve_mode(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    tex::textree::init(&cfg.tex, None)?;
    let env = cfg.tex.clone();
    let compile_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_COMPILES));
    let api = Router::new()
        .route(
            "/tex",
            post(move |request| {
                tex::handle_tex(
                    request,
                    env.clone(),
                    Some(MAX_BODY_SIZE),
                    Some(compile_slots.clone()),
                )
            }),
        )
        // Unknown API paths must not fall through to the editor fallback.
        .fallback(|| async { StatusCode::NOT_FOUND });
    let router = Router::new()
        .nest("/api", api)
        // `nest` does not cover the trailing-slash path.
        .route("/api/", any(|| async { StatusCode::NOT_FOUND }))
        // Other paths serve the editor; app routes like /doc/42 load its index.html.
        .fallback(|request| handle_embedded(request, Web, Some("index.html")))
        // Accept requests from any origin, with any method and headers.
        .layer(CorsLayer::permissive());
    let mut server = Server::new(cfg.server.bind_address());
    server.set_router(router);
    server.listen().await
}

fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    match config.mode {
        Mode::Build => build_mode(config),
        Mode::Install => Ok(tex::textree::init(&config.tex, Some(config.force))?),
        Mode::Serve => serve_mode(config),
    }
}

fn main() {
    if let Err(err) = Config::use_args().and_then(|config| run(&config)) {
        eprintln!("ERROR: {err}");
        std::process::exit(1);
    }
}
