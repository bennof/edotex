use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
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
use tower_http::cors::{Any, CorsLayer};

#[cfg(all(feature = "desktop", any(target_os = "macos", target_os = "windows")))]
mod tray;

/// Maximum size of a TeX request body (20 MiB).
const MAX_BODY_SIZE: usize = 20 * 1024 * 1024;
/// Maximum number of TeX compilations running at the same time.
const MAX_CONCURRENT_COMPILES: usize = 4;

/// The editor web app, built into `web/build/` before compiling.
#[derive(Embed)]
#[folder = "web/build/"]
struct Web;

/// Compiles `cfg.input` and writes the PDF to `cfg.output` or next to the input.
/// An output path ending in `.png` gets the first page as PNG instead.
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
    let bytes = if is_png(&output_path) {
        render_png(out.pdf, 0)?
    } else {
        out.pdf
    };
    println!("write: {}", output_path.display());
    fs::write(output_path, bytes)?;

    Ok(())
}

/// Renders one page of the PDF `cfg.input` (`cfg.page`, default 1) and writes a PNG
/// to `cfg.output` or next to the input.
fn pdf2png_mode(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let input_path = cfg
        .input
        .as_deref()
        .ok_or("pdf2png mode needs an input .pdf file")?;
    let page = cfg.page.unwrap_or(1).saturating_sub(1) as usize;
    let png = render_png(fs::read(input_path)?, page)?;

    let output_path = match &cfg.output {
        Some(path) => path.clone(),
        None => png_output_path(input_path),
    };
    println!("write: {}", output_path.display());
    fs::write(output_path, png)?;

    Ok(())
}

/// Render scale for PNG output; 2.0 ≈ 144 dpi.
#[cfg(feature = "png")]
const PNG_SCALE: f32 = 2.0;

/// Whether `path` asks for a PNG, judged by its extension.
fn is_png(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
}

/// Renders page `page` (0-based) of `pdf` as PNG.
#[cfg(feature = "png")]
fn render_png(pdf: Vec<u8>, page: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    edotex::png::pdf_to_png(pdf, page, PNG_SCALE).ok_or_else(|| {
        format!(
            "cannot render page {} (not a readable PDF or no such page)",
            page + 1
        )
        .into()
    })
}

#[cfg(not(feature = "png"))]
fn render_png(_pdf: Vec<u8>, _page: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Err("PNG output needs edotex built with the feature \"png\"".into())
}

fn pdf_output_path(input_path: &Path) -> PathBuf {
    let mut output_path = input_path.to_path_buf();
    output_path.set_extension("pdf");
    output_path
}

fn png_output_path(input_path: &Path) -> PathBuf {
    let mut output_path = input_path.to_path_buf();
    output_path.set_extension("png");
    output_path
}

#[tokio::main]
async fn serve_mode(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    tex::textree::init(&cfg.tex, None)?;
    let env = cfg.tex.clone();
    let compile_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_COMPILES));
    let api = Router::new().route(
        "/tex",
        post(move |request| {
            tex::handle_tex(
                request,
                env.clone(),
                Some(MAX_BODY_SIZE),
                Some(compile_slots.clone()),
            )
        }),
    );
    // POST a PDF, get its first page as PNG.
    #[cfg(feature = "png")]
    let api = api.route(
        "/png",
        post(|request| edotex::png::handler::handle_png(request, Some(MAX_BODY_SIZE))),
    );
    // Unknown API paths must not fall through to the editor fallback.
    let api = api.fallback(|| async { StatusCode::NOT_FOUND });
    let router = Router::new()
        .nest("/api", api)
        // `nest` does not cover the trailing-slash path.
        .route("/api/", any(|| async { StatusCode::NOT_FOUND }))
        // Other paths serve the editor; app routes like /doc/42 load its index.html.
        .fallback(|request| handle_embedded(request, Web, Some("index.html")))
        // Accept requests from any origin and method. Allowed headers are listed
        // explicitly: the wildcard "*" does not cover Authorization (bearer tokens).
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers([AUTHORIZATION, CONTENT_TYPE])
                .expose_headers(Any),
        );
    let mut server = Server::new(cfg.server.bind_address());
    server.set_router(router);
    server.listen().await
}

/// Runs the server with a menu bar (tray) icon; only in desktop builds.
fn tray_mode(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(feature = "desktop", any(target_os = "macos", target_os = "windows")))]
    return tray::run(cfg, serve_mode);
    #[cfg(not(all(feature = "desktop", any(target_os = "macos", target_os = "windows"))))]
    {
        let _ = cfg;
        Err("tray mode is not available in this build".into())
    }
}

/// Whether the binary was double-clicked as a macOS app (no arguments, inside an `.app`).
fn launched_as_app(cfg: &Config) -> bool {
    #[cfg(all(feature = "desktop", target_os = "macos"))]
    return matches!(cfg.mode, Mode::Build)
        && cfg.input.is_none()
        && tray::launched_from_app_bundle();
    #[cfg(not(all(feature = "desktop", target_os = "macos")))]
    {
        let _ = cfg;
        false
    }
}

fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if launched_as_app(config) {
        return tray_mode(config);
    }
    match config.mode {
        Mode::Build => build_mode(config),
        Mode::Install => Ok(tex::textree::init(&config.tex, Some(config.force))?),
        Mode::Serve => serve_mode(config),
        Mode::Pdf2png => pdf2png_mode(config),
        Mode::Tray => tray_mode(config),
    }
}

fn main() {
    if let Err(err) = Config::use_args().and_then(|config| run(&config)) {
        eprintln!("ERROR: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_output_is_chosen_by_extension() {
        assert!(is_png(Path::new("figure.png")));
        assert!(is_png(Path::new("dir/figure.PNG")));
        assert!(!is_png(Path::new("exam.pdf")));
        assert!(!is_png(Path::new("png")));
    }

    #[test]
    fn default_output_paths_replace_the_extension() {
        assert_eq!(
            pdf_output_path(Path::new("a/exam.tex")),
            Path::new("a/exam.pdf")
        );
        assert_eq!(
            png_output_path(Path::new("a/exam.pdf")),
            Path::new("a/exam.png")
        );
    }

    #[test]
    fn render_png_rejects_what_is_no_pdf() {
        assert!(render_png(b"no pdf".to_vec(), 0).is_err());
    }
}
