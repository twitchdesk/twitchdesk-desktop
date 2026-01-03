use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::preview;

const LOADING_HTML: &str = include_str!("../assets/twitchdesk-loading.html");

fn loading_html_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("com", "TwitchDesk", "TwitchDesk")
        .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
    let dir = proj.cache_dir().join("loading");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir.join("twitchdesk-loading.html"))
}

fn path_to_file_url(path: &std::path::Path) -> Result<String> {
    let p = path.canonicalize().with_context(|| format!("canonicalize {}", path.display()))?;
    let mut s = p.to_string_lossy().replace('\\', "/");

    // Handle Windows extended-length paths.
    if let Some(stripped) = s.strip_prefix("//?/" ) {
        s = stripped.to_string();
    }

    // Basic percent-encoding for file URLs.
    let mut encoded = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        let keep = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~' | '/' | ':');
        if keep {
            encoded.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).as_bytes() {
                encoded.push('%');
                encoded.push_str(&format!("{:02X}", b));
            }
        }
    }

    if encoded.len() >= 2 && encoded.as_bytes()[1] == b':' {
        // Drive letter path, e.g. D:/...
        Ok(format!("file:///{encoded}"))
    } else {
        Ok(format!("file://{encoded}"))
    }
}

pub(crate) fn show_startup_loading_splash_best_effort() {
    if let Err(e) = try_show_startup_loading_splash() {
        tracing::warn!("loading splash disabled: {e:#}");
    }
}

fn try_show_startup_loading_splash() -> Result<()> {
    let path = loading_html_path()?;
    fs::write(&path, LOADING_HTML).with_context(|| format!("write {}", path.display()))?;

    let url = path_to_file_url(&path)?;

    // Keep it short: show the brand loading, then start the app.
    let extra_args = vec!["--auto-close-ms".to_string(), "3000".to_string()];
    preview::open_preview_with_args_blocking(&url, &extra_args)
}
