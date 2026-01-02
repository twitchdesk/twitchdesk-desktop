use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use semver::Version;
use serde::Deserialize;
use tracing::{info, warn};

const DEFAULT_GITHUB_OWNER: &str = "twitchdesk";
const DEFAULT_GITHUB_REPO: &str = "twitchdesk-desktop";

const ARG_APPLY_UPDATE: &str = "--apply-update";
const ARG_TARGET_EXE: &str = "--target-exe";
const ARG_SKIP_UPDATE: &str = "--skip-update";
const ARG_RELAUNCH_SEP: &str = "--";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub(crate) fn maybe_run_startup_update() -> Result<()> {
    // Skip in debug to keep dev fast.
    if cfg!(debug_assertions) {
        return Ok(());
    }

    if std::env::var("TWITCHDESK_DISABLE_UPDATES")
        .ok()
        .as_deref()
        == Some("1")
    {
        return Ok(());
    }

    let args = std::env::args().collect::<Vec<_>>();

    // Internal mode: apply downloaded update and relaunch.
    if let Some(pos) = args.iter().position(|a| a == ARG_APPLY_UPDATE) {
        let downloaded = args
            .get(pos + 1)
            .map(|s| s.as_str())
            .context("missing path after --apply-update")?;

        let target_exe = if let Some(tp) = args.iter().position(|a| a == ARG_TARGET_EXE) {
            args.get(tp + 1)
                .map(|s| s.as_str())
                .context("missing path after --target-exe")?
        } else {
            // Backwards-compatible default: use current exe.
            ""
        };

        let target_exe = if target_exe.is_empty() {
            std::env::current_exe().context("get current exe")?
        } else {
            PathBuf::from(target_exe)
        };

        // Relaunch args = everything after "--".
        let relaunch_args = if let Some(sep) = args.iter().position(|a| a == ARG_RELAUNCH_SEP) {
            args.iter().skip(sep + 1).cloned().collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        apply_update_and_relaunch(&target_exe, Path::new(downloaded), &relaunch_args)?;
        // If we succeed, we should not continue into normal UI.
        std::process::exit(0);
    }

    // User-visible mode: optionally skip once.
    if args.iter().any(|a| a == ARG_SKIP_UPDATE) {
        return Ok(());
    }

    // Check latest GitHub release, download correct asset, then apply+relaunch.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create tokio runtime for updater")?;

    if let Some(downloaded) = rt.block_on(download_latest_if_newer())? {
        info!(path = %downloaded.display(), "downloaded update; applying");

        let current_exe = std::env::current_exe().context("get current exe")?;
        let helper = ensure_updater_helper(&current_exe)?;

        // Relaunch args: original args (excluding program path) + skip flag.
        let mut relaunch_args = args.into_iter().skip(1).collect::<Vec<_>>();
        relaunch_args.push(ARG_SKIP_UPDATE.to_string());

        // Run helper from a copy so it can replace the real executable (Windows locks running exe).
        Command::new(&helper)
            .arg(ARG_APPLY_UPDATE)
            .arg(downloaded)
            .arg(ARG_TARGET_EXE)
            .arg(current_exe)
            .arg(ARG_RELAUNCH_SEP)
            .args(relaunch_args)
            .spawn()
            .context("spawn apply-update helper")?;

        std::process::exit(0);
    }

    Ok(())
}

fn ensure_updater_helper(current_exe: &Path) -> Result<PathBuf> {
    let Some(proj) = ProjectDirs::from("com", "twitchdesk", "TwitchDesk") else {
        anyhow::bail!("unable to resolve user directories")
    };
    let dir = proj.cache_dir().join("updates").join("helper");
    std::fs::create_dir_all(&dir).with_context(|| format!("create dir {}", dir.display()))?;

    let ext = if std::env::consts::OS == "windows" { ".exe" } else { "" };
    let helper = dir.join(format!("twitchdesk-desktop-updater{ext}"));

    // Always refresh helper to match current version.
    let _ = std::fs::remove_file(&helper);
    std::fs::copy(current_exe, &helper)
        .with_context(|| format!("copy updater helper to {}", helper.display()))?;

    Ok(helper)
}

async fn download_latest_if_newer() -> Result<Option<PathBuf>> {
    let (owner, repo) = github_repo_from_env();

    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .context("parse current app version")?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .context("build http client")?;

    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/releases/latest"
    );

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "twitchdesk-desktop-updater")
        .send()
        .await
        .context("fetch latest release")?;

    if !resp.status().is_success() {
        warn!(status = %resp.status(), "update check failed");
        return Ok(None);
    }

    let release: GithubRelease = resp.json().await.context("parse release json")?;

    let latest = Version::parse(release.tag_name.trim_start_matches('v'))
        .context("parse latest version")?;

    if latest <= current {
        return Ok(None);
    }

    let asset_name = expected_asset_name();

    let Some(asset) = release.assets.into_iter().find(|a| a.name == asset_name) else {
        warn!(expected = %asset_name, "no matching release asset for this platform");
        return Ok(None);
    };

    info!(from = %current, to = %latest, asset = %asset_name, "update available");

    let bytes = client
        .get(asset.browser_download_url)
        .header(reqwest::header::USER_AGENT, "twitchdesk-desktop-updater")
        .send()
        .await
        .context("download release asset")?
        .bytes()
        .await
        .context("read asset bytes")?;

    let path = update_download_path(&latest, &asset_name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::write(&path, &bytes).with_context(|| format!("write {}", path.display()))?;

    Ok(Some(path))
}

fn github_repo_from_env() -> (String, String) {
    // Optional override: TWITCHDESK_UPDATE_REPO="owner/repo"
    if let Ok(v) = std::env::var("TWITCHDESK_UPDATE_REPO") {
        if let Some((o, r)) = v.split_once('/') {
            if !o.trim().is_empty() && !r.trim().is_empty() {
                return (o.trim().to_string(), r.trim().to_string());
            }
        }
    }
    (DEFAULT_GITHUB_OWNER.to_string(), DEFAULT_GITHUB_REPO.to_string())
}

fn expected_asset_name() -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "macos",
        other => other,
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => other,
    };

    let ext = if os == "windows" { ".exe" } else { "" };

    format!("twitchdesk-desktop-{os}-{arch}{ext}")
}

fn update_download_path(version: &Version, asset_name: &str) -> Result<PathBuf> {
    let Some(proj) = ProjectDirs::from("com", "twitchdesk", "TwitchDesk") else {
        anyhow::bail!("unable to resolve user directories")
    };

    Ok(proj
        .cache_dir()
        .join("updates")
        .join(version.to_string())
        .join(asset_name))
}

fn apply_update_and_relaunch(target_exe: &Path, downloaded: &Path, relaunch_args: &[String]) -> Result<()> {

    // Wait a bit for the parent process (that spawned us) to fully exit.
    // Especially needed on Windows where the .exe is locked while running.
    for _ in 0..30 {
        if try_apply_update(target_exe, downloaded).is_ok() {
            info!("update applied");

            // Relaunch updated binary.
            let mut cmd = Command::new(target_exe);
            for arg in relaunch_args {
                cmd.arg(arg);
            }
            cmd.spawn().context("relaunch after update")?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    warn!("failed to apply update after retries; continuing without updating");
    Ok(())
}

fn try_apply_update(exe_path: &Path, downloaded: &Path) -> Result<()> {
    if !downloaded.exists() {
        anyhow::bail!("downloaded update missing")
    }

    let exe_file = exe_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("twitchdesk-desktop");

    let old_path = exe_path.with_file_name(format!("{exe_file}.old"));

    // Best-effort cleanup.
    let _ = std::fs::remove_file(&old_path);

    // Swap: exe -> old, downloaded -> exe.
    std::fs::rename(exe_path, &old_path).context("rename current exe to .old")?;
    std::fs::rename(downloaded, exe_path).context("rename downloaded to current exe")?;

    Ok(())
}
