use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

pub(crate) fn open_preview(url: &str) -> Result<()> {
    open_preview_with_args(url, &[])
}

pub(crate) fn open_preview_with_args(url: &str, extra_args: &[String]) -> Result<()> {
    let current_exe = std::env::current_exe().context("get current exe")?;
    let exe_dir = current_exe
        .parent()
        .context("resolve exe directory")?
        .to_path_buf();

    let preview_exe_name = if std::env::consts::OS == "windows" {
        "twitchdesk-preview.exe"
    } else {
        "twitchdesk-preview"
    };

    // Release installs should place the helper next to the main app.
    let candidate = exe_dir.join(preview_exe_name);
    let preview_exe = if candidate.exists() {
        candidate
    } else {
        // Dev fallback: try cargo-built path
        let mut p = PathBuf::from("target");
        p.push("debug");
        p.push(preview_exe_name);
        p
    };

    if !preview_exe.exists() {
        anyhow::bail!(
            "preview helper not found (expected {}). Reinstall from release zip.",
            preview_exe.display()
        );
    }

    let mut cmd = Command::new(preview_exe);
    cmd.arg(url);
    cmd.args(extra_args);
    cmd.spawn().context("launch preview helper")?;

    Ok(())
}
