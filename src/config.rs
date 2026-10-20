// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Benjamin Benno Falkner

use crate::tex::TeX_Env;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Runtime mode selected on the command line.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ValueEnum)]
pub enum Mode {
    /// Build a TeX input.
    #[default]
    Build,
    /// Install or prepare local TeX resources.
    Install,
    /// Start the edotex server.
    Serve,
    /// Render one page of a PDF as PNG.
    Pdf2png,
}

/// Server binding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// TCP port used by the server.
    pub port: u16,
    /// Host name or address used by the server.
    pub server_name: String,
}

impl ServerConfig {
    /// Returns the server bind address as `host:port`.
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server_name, self.port)
    }
}

/// Complete runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Runtime mode selected for this invocation.
    #[serde(skip)]
    pub mode: Mode,
    /// Optional input path for modes that need one.
    #[serde(skip)]
    pub input: Option<PathBuf>,
    /// Optional output path override for build and pdf2png mode.
    #[serde(skip)]
    pub output: Option<PathBuf>,
    /// Page for pdf2png mode, starting at 1; `None` means the first page.
    #[serde(skip)]
    pub page: Option<u32>,
    /// TeX code supplied on the command line, prepended to the document.
    #[serde(skip)]
    pub inject: Option<String>,
    /// Whether to force reinitializing the local TeX tree.
    #[serde(skip)]
    pub force: bool,
    /// Server-related settings.
    pub server: ServerConfig,
    /// TeX-related paths.
    pub tex: TeX_Env,
}

/// Optional TeX values read from JSON.
#[derive(Debug, Default, Deserialize)]
struct PartialTexConfig {
    local_dir: Option<PathBuf>,
    tree_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
    search_paths: Option<Vec<PathBuf>>,
}

/// Optional server values read from JSON.
#[derive(Debug, Default, Deserialize)]
struct PartialServerConfig {
    port: Option<u16>,
    server_name: Option<String>,
}

/// Optional top-level values read from JSON.
#[derive(Debug, Default, Deserialize)]
struct PartialConfig {
    server: Option<PartialServerConfig>,
    tex: Option<PartialTexConfig>,
}

/// Command-line overrides for the configuration.
#[derive(Debug, Parser)]
#[command(version, about = "edotex TeX compiler and HTTP server")]
struct Args {
    #[arg(
        index = 1,
        value_name = "MODE",
        help = "Runtime mode: build, install, serve, or pdf2png. Defaults to build."
    )]
    mode_or_input: Option<String>,

    #[arg(
        index = 2,
        value_name = "INPUT",
        help = "Optional input file or directory, depending on the selected mode."
    )]
    input: Option<PathBuf>,

    #[arg(
        short = 'i',
        long = "inject",
        value_name = "CODE",
        help = "TeX code to prepend before the document."
    )]
    inject: Option<String>,

    #[arg(
        short = 'c',
        long = "config",
        default_value = "./config.json",
        help = "Path to a JSON config file. If it exists, its values override the defaults."
    )]
    config: Option<PathBuf>,

    #[arg(
        long,
        help = "Write the default configuration to this JSON file and exit."
    )]
    write_config: Option<PathBuf>,

    #[arg(
        short = 'p',
        long = "port",
        help = "Server port. Overrides the value from the config file."
    )]
    server_port: Option<u16>,

    #[arg(
        long = "host",
        help = "Server host name or address. Overrides the value from the config file."
    )]
    server_name: Option<String>,

    #[arg(
        long = "local-dir",
        help = "Base directory for edotex data. Overrides the value from the config file."
    )]
    tex_local_dir: Option<PathBuf>,

    #[arg(
        long = "texmf",
        help = "Path to the TeX tree directory. Overrides the value from the config file."
    )]
    tex_tree_path: Option<PathBuf>,

    #[arg(
        long = "tectonic-cache",
        help = "Path to the Tectonic cache directory. Overrides the value from the config file."
    )]
    tex_cache_path: Option<PathBuf>,

    #[arg(
        long = "tex-search-path",
        value_name = "PATH",
        help = "Additional TeX input search path. Can be supplied multiple times."
    )]
    tex_search_paths: Vec<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        help = "Output path. build: .pdf, or .png for the first page as image; pdf2png: .png. \
                Defaults to the input path with its extension changed to .pdf (build) or .png (pdf2png)."
    )]
    output: Option<PathBuf>,

    #[arg(
        short = 's',
        long = "page",
        value_name = "PAGE",
        value_parser = clap::value_parser!(u32).range(1..),
        help = "Page to render in pdf2png mode, starting at 1. Defaults to 1."
    )]
    page: Option<u32>,
    #[arg(short = 'f', long = "force", help = "force overwrite current settings")]
    force: bool,
}

impl Default for Config {
    /// Builds the default configuration.
    fn default() -> Self {
        let local_dir = default_local_dir();

        Self {
            mode: Mode::Build,
            input: None,
            output: None,
            page: None,
            inject: None,
            force: false,
            server: ServerConfig {
                port: 8080,
                server_name: "localhost".to_string(),
            },
            tex: TeX_Env {
                local_dir,
                tree_path: None,
                cache_path: None,
                search_paths: Vec::new(),
            },
        }
    }
}

impl Config {
    /// Reads a JSON config file and applies it over the defaults.
    pub fn read_json(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let partial_config: PartialConfig = serde_json::from_str(&content)?;
        let mut config = Config::default();
        config.apply_partial(partial_config);
        Ok(config)
    }

    /// Writes the default configuration to a JSON file.
    pub fn write_json(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
        let config = Config::default();
        let content = serde_json::to_string_pretty(&config)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(config)
    }

    /// Loads defaults, overlays JSON config if present, then applies CLI arguments.
    pub fn use_args() -> Result<Config, Box<dyn std::error::Error>> {
        Self::from_args(Args::parse())
    }

    fn from_args(args: Args) -> Result<Config, Box<dyn std::error::Error>> {
        if let Some(path) = args.write_config.as_deref() {
            return Config::write_json(path);
        }

        let mut config = if let Some(path) = args.config.as_deref().filter(|path| path.exists()) {
            Config::read_json(path)?
        } else {
            Config::default()
        };

        let (mode, input) = args.mode_and_input()?;
        config.mode = mode;
        config.input = input;
        config.apply_args(args);
        Ok(config)
    }

    /// Applies optional values from a JSON config.
    fn apply_partial(&mut self, partial_config: PartialConfig) {
        if let Some(server) = partial_config.server {
            if let Some(port) = server.port {
                self.server.port = port;
            }
            if let Some(server_name) = server.server_name {
                self.server.server_name = server_name;
            }
        }

        if let Some(tex) = partial_config.tex {
            if let Some(local_dir) = tex.local_dir {
                self.tex.local_dir = local_dir;
            }
            if let Some(tree_path) = tex.tree_path {
                self.tex.tree_path = Some(tree_path);
            }
            if let Some(cache_path) = tex.cache_path {
                self.tex.cache_path = Some(cache_path);
            }
            if let Some(search_paths) = tex.search_paths {
                self.tex.search_paths = search_paths;
            }
        }
    }

    /// Applies values explicitly provided on the command line.
    fn apply_args(&mut self, args: Args) {
        self.inject = args.inject;
        self.force = args.force;
        self.output = args.output;
        self.page = args.page;
        if let Some(port) = args.server_port {
            self.server.port = port;
        }
        if let Some(server_name) = args.server_name {
            self.server.server_name = server_name;
        }
        if let Some(local_dir) = args.tex_local_dir {
            self.tex.local_dir = local_dir;
        }
        if let Some(tree_path) = args.tex_tree_path {
            self.tex.tree_path = Some(tree_path);
        }
        if let Some(cache_path) = args.tex_cache_path {
            self.tex.cache_path = Some(cache_path);
        }
        if !args.tex_search_paths.is_empty() {
            self.tex.search_paths = args.tex_search_paths;
        }
    }
}

impl Args {
    fn mode_and_input(&self) -> Result<(Mode, Option<PathBuf>), Box<dyn std::error::Error>> {
        let Some(first) = &self.mode_or_input else {
            return Ok((Mode::Build, None));
        };

        if let Ok(mode) = Mode::from_str(first, true) {
            return Ok((mode, self.input.clone()));
        }

        if self.input.is_none() {
            return Ok((Mode::Build, Some(PathBuf::from(first))));
        }

        Err(format!("unknown mode: {first}").into())
    }
}

/// Returns the platform-specific default data directory.
fn default_local_dir() -> PathBuf {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/edotex");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("edotex");
        }
    }

    PathBuf::from(".edotex")
}
