// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Benjamin Benno Falkner

//! Start at login.
//!
//! macOS: a LaunchAgent in `~/Library/LaunchAgents` opens the app bundle at login.
//! Windows: not implemented yet (an MSIX package declares a startup task instead).

use std::{io, path::PathBuf};

#[cfg(target_os = "macos")]
const LABEL: &str = "xyz.falkner.edotex";

/// Returns the enclosing `.app` bundle when running from `X.app/Contents/MacOS/`.
pub fn app_bundle_path() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let macos_dir = exe.parent()?;
    let contents = macos_dir.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos_dir.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app";
    is_bundle.then(|| bundle.to_path_buf())
}

/// Whether start at login can be switched on this platform.
pub fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(target_os = "macos")]
fn agent_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(format!("Library/LaunchAgents/{LABEL}.plist")))
}

/// Whether start at login is switched on.
pub fn is_enabled() -> bool {
    #[cfg(target_os = "macos")]
    return agent_path().is_some_and(|path| path.exists());
    #[cfg(not(target_os = "macos"))]
    false
}

/// Switches start at login on or off. Takes effect at the next login.
pub fn set_enabled(enable: bool) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let path = agent_path().ok_or_else(|| io::Error::other("HOME is not set"))?;
        if !enable {
            return match std::fs::remove_file(&path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            };
        }
        // Opening the bundle keeps macOS's single-instance handling;
        // outside a bundle, start the binary in tray mode.
        let program = match app_bundle_path() {
            Some(bundle) => vec!["/usr/bin/open".into(), bundle.display().to_string()],
            None => vec![
                std::env::current_exe()?.display().to_string(),
                "tray".into(),
            ],
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, launch_agent_plist(&program))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enable;
        Err(io::Error::other("start at login is not supported here"))
    }
}

#[cfg(target_os = "macos")]
fn launch_agent_plist(program: &[String]) -> String {
    let args: String = program
        .iter()
        .map(|arg| format!("        <string>{}</string>\n", xml_escape(arg)))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
{args}    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_escapes_paths() {
        let plist = launch_agent_plist(&["/usr/bin/open".into(), "/Apps/A&B.app".into()]);
        assert!(plist.contains("<string>/Apps/A&amp;B.app</string>"));
        assert!(plist.contains("<string>xyz.falkner.edotex</string>"));
    }
}
