use include_dir::{Dir, include_dir};
use std::fs;
use std::path::Path;

use super::TeX_Env;

/// TeX tree embedded into the edotex binary.
pub static TEX_FILES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/texmf");

/// Initializes the local TeX tree configured in `env`.
pub fn init(env: &TeX_Env, force: Option<bool>) -> std::io::Result<()> {
    let tree_path = env.tree_path();

    if !force.unwrap_or(false) && tree_path_exists(&tree_path)? {
        return Ok(());
    }

    fs::create_dir_all(&tree_path)?;
    extract_tex_files(&tree_path)?;
    gen_fontconfig_tex(&tree_path)
}

/// Extracts the embedded TeX tree into `target`.
pub fn extract_tex_files(target: &Path) -> std::io::Result<()> {
    extract_dir(&TEX_FILES, target)
}

pub fn gen_fontconfig_tex(target: &Path) -> std::io::Result<()> {
    let path = target.join("tex/latex/bflatex/fontconfig.tex");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = format!(
        "\\useTeXTreetrue\n\\newcommand{{\\fontpath}}{{{}/}}\n",
        target.display()
    );

    fs::write(path, content)
}

/// Recursively extracts an embedded directory below `target`.
pub fn extract_dir(dir: &Dir<'_>, target: &Path) -> std::io::Result<()> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(dir) => {
                let path = target.join(dir.path());
                fs::create_dir_all(&path)?;
                extract_dir(dir, target)?;
            }

            include_dir::DirEntry::File(file) => {
                let path = target.join(file.path());

                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(path, file.contents())?;
            }
        }
    }
    Ok(())
}

fn tree_path_exists(path: &Path) -> std::io::Result<bool> {
    match fs::read_dir(path) {
        Ok(mut entries) => Ok(entries.next().is_some()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}
