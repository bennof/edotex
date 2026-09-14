// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Benjamin Benno Falkner

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tectonic::Error as TectonicError;
use tectonic::config::PersistentConfig;
use tectonic::driver::{OutputFormat, ProcessingSessionBuilder};
use tectonic::status::{MessageKind, StatusBackend};
use tectonic::unstable_opts::UnstableOptions;
use tectonic_bridge_core::{SecuritySettings, SecurityStance};

const DEFAULT_DIR: &str = "edotex";
const INPUT_NAME: &str = "texput.tex";
const OUTPUT_NAME: &str = "texput.pdf";

/// Paths used by the embedded TeX compiler.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeX_Env {
    /// Base directory for local TeX data.
    pub local_dir: PathBuf,
    /// Local TeX tree directory.
    pub tree_path: Option<PathBuf>,
    /// Tectonic format cache directory.
    pub cache_path: Option<PathBuf>,
    /// Additional directories searched for TeX inputs.
    pub search_paths: Vec<PathBuf>,
}

impl Default for TeX_Env {
    fn default() -> Self {
        Self {
            local_dir: default_local_dir().join(DEFAULT_DIR),
            tree_path: None,
            cache_path: None,
            search_paths: Vec::new(),
        }
    }
}

impl TeX_Env {
    /// Returns the local TeX tree path, using the default below `local_dir`.
    pub fn tree_path(&self) -> PathBuf {
        self.tree_path
            .clone()
            .unwrap_or_else(|| self.local_dir.join("texmf"))
    }

    /// Returns the format cache path, using the default below `local_dir`.
    pub fn cache_path(&self) -> PathBuf {
        self.cache_path
            .clone()
            .unwrap_or_else(|| self.local_dir.join("tectonic-cache"))
    }

    /// Compatibility helper for callers that used the original sketch.
    pub fn get_tree_path(&self) -> PathBuf {
        self.tree_path()
    }

    /// Compatibility helper for callers that used the original sketch.
    pub fn get_cache_path(&self) -> PathBuf {
        self.cache_path()
    }

    /// Builds Tectonic's unstable option set for additional search paths.
    pub fn get_search_path(&self) -> UnstableOptions {
        let mut opts = UnstableOptions::default();
        let mut seen = HashSet::new();

        add_search_path(&mut opts.extra_search_paths, &mut seen, &self.tree_path());
        for path in &self.search_paths {
            add_search_path(&mut opts.extra_search_paths, &mut seen, path);
        }

        opts
    }
}

fn add_search_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: &Path) {
    if !path.is_dir() {
        return;
    }

    let path = path.to_path_buf();
    if seen.insert(path.clone()) {
        paths.push(path.clone());
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        add_search_path(paths, seen, &entry.path());
    }
}

/// Returns the platform-specific default data directory.
fn default_local_dir() -> PathBuf {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local);
        }
    }

    PathBuf::from(".")
}

/// Successful compiler output.
#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub struct TeX_Output {
    pub pdf: Vec<u8>,
    pub output_log: String,
}

/// Compatibility alias for the misspelled name used in an earlier sketch.
#[allow(non_camel_case_types)]
pub type TeX_Outup = TeX_Output;

/// Failed compiler output with the stdout produced before the error.
#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub struct TeX_Error {
    pub message: String,
    pub output_log: String,
}

impl TeX_Error {
    fn new(message: impl Into<String>, output_log: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            output_log: output_log.into(),
        }
    }
}

impl fmt::Display for TeX_Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TeX_Error {}

/// Compiles a complete LaTeX document with Tectonic.
///
/// Nonempty `inject` code is prepended to the input, followed by a newline.
/// It runs before the document, for example to define configuration macros.
pub fn compile<W: Write>(
    env: &TeX_Env,
    input: Vec<u8>,
    out: &mut W,
    inject: Option<&str>,
) -> Result<TeX_Output, TeX_Error> {
    let input = match inject {
        Some(code) if !code.is_empty() => {
            let mut combined = Vec::with_capacity(code.len() + 1 + input.len());
            combined.extend_from_slice(code.as_bytes());
            combined.push(b'\n');
            combined.extend_from_slice(&input);
            combined
        }
        _ => input,
    };

    let cache_path = env.cache_path();
    ensure_dir(&cache_path).map_err(|err| TeX_Error::new(err.to_string(), ""))?;

    let mut status = CaptureStatus::new(out);
    let config =
        PersistentConfig::open(false).map_err(|err| TeX_Error::new(err.to_string(), ""))?;
    let bundle = config
        .default_bundle(false)
        .map_err(|err| TeX_Error::new(err.to_string(), ""))?;

    let security = SecuritySettings::new(SecurityStance::MaybeAllowInsecures);
    let mut builder = ProcessingSessionBuilder::new_with_security(security);
    builder
        .bundle(bundle)
        .primary_input_buffer(&input)
        .tex_input_name(INPUT_NAME)
        .format_name("latex")
        .format_cache_path(&cache_path)
        .unstables(env.get_search_path())
        .keep_logs(true)
        .keep_intermediates(false)
        .print_stdout(false)
        .output_format(OutputFormat::Pdf)
        .do_not_write_output_files();

    let mut session = match builder.create(&mut status) {
        Ok(session) => session,
        Err(err) => return Err(TeX_Error::new(err.to_string(), status.finish())),
    };

    if let Err(err) = session.run(&mut status) {
        let engine_stdout = session.get_stdout_content();
        status.write_bytes(&engine_stdout);
        let output_log = status.finish();
        return Err(TeX_Error::new(err.to_string(), output_log));
    }

    let engine_stdout = session.get_stdout_content();
    status.write_bytes(&engine_stdout);
    let output_log = status.finish();
    let mut files = session.into_file_data();
    let pdf = files
        .remove(OUTPUT_NAME)
        .map(|file| file.data)
        .ok_or_else(|| {
            TeX_Error::new(
                format!("Tectonic did not create {OUTPUT_NAME}"),
                output_log.clone(),
            )
        })?;

    Ok(TeX_Output { pdf, output_log })
}

fn ensure_dir(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(path)?;
    Ok(())
}

struct CaptureStatus<'a, W: Write> {
    log: String,
    out: &'a mut W,
    write_error: Option<io::Error>,
}

impl<'a, W: Write> CaptureStatus<'a, W> {
    fn new(out: &'a mut W) -> Self {
        Self {
            log: String::new(),
            out,
            write_error: None,
        }
    }

    fn finish(self) -> String {
        self.log
    }

    fn push_line(&mut self, line: impl AsRef<str>) {
        let line = line.as_ref();
        self.write_bytes(line.as_bytes());
        self.write_bytes(b"\n");
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.log.push_str(&String::from_utf8_lossy(bytes));

        if self.write_error.is_some() {
            return;
        }

        if let Err(err) = self.out.write_all(bytes) {
            self.write_error = Some(err);
        }
    }
}

impl<W: Write> StatusBackend for CaptureStatus<'_, W> {
    fn report(&mut self, kind: MessageKind, args: fmt::Arguments<'_>, err: Option<&TectonicError>) {
        let prefix = match kind {
            MessageKind::Note => "note",
            MessageKind::Warning => "warning",
            MessageKind::Error => "error",
        };

        self.push_line(format!("{prefix}: {args}"));

        if let Some(err) = err {
            for item in err.chain() {
                self.push_line(format!("caused by: {item}"));
            }
        }
    }

    fn report_error(&mut self, err: &TectonicError) {
        let mut prefix = "error";

        for item in err.chain() {
            self.push_line(format!("{prefix}: {item}"));
            prefix = "caused by";
        }
    }

    fn dump_error_logs(&mut self, output: &[u8]) {
        self.write_bytes(output);
        if !self.log.ends_with('\n') {
            self.write_bytes(b"\n");
        }
    }
}
