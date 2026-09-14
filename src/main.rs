use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use edotex::config::{self, Mode};
use edotex::tex::{self, compile};

fn build_mode(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    tex::textree::init(&cfg.tex)?;

    let input_path = cfg
        .input
        .as_deref()
        .ok_or("build mode needs an input .tex file")?;
    let input = fs::read(input_path)?;

    let mut stdout = io::stdout();
    let out = match compile(&cfg.tex, input, &mut stdout, cfg.inject.as_deref()) {
        Ok(out) => out,
        Err(err) => {
            return Err(Box::new(err));
        }
    };

    let output_path = pdf_output_path(input_path);
    println!("write: {}", output_path.display());
    fs::write(output_path, out.pdf)?;

    Ok(())
}

fn pdf_output_path(input_path: &Path) -> PathBuf {
    let mut output_path = input_path.to_path_buf();
    output_path.set_extension("pdf");
    output_path
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::use_args()?;

    match config.mode {
        Mode::Build => build_mode(&config)?,
        Mode::Install => tex::textree::init(&config.tex)?,
        Mode::Serve => return Err("serve mode is not implemented yet".into()),
    }

    Ok(())
}

/// Prints fatal errors and returns a non-zero exit status.
fn main() {
    if let Err(err) = run() {
        eprintln!("ERROR: {err}");
        std::process::exit(1);
    }
}
