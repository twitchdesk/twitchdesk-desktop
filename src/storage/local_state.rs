use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::models::LocalClientState;

pub fn local_state_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("com", "TwitchDesk", "TwitchDesk")
        .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
    let dir = proj.data_local_dir();
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir.join("local-state.json"))
}

pub fn load_local_state() -> Result<LocalClientState> {
    let path = local_state_path()?;
    if !path.exists() {
        return Ok(LocalClientState::default());
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let st = serde_json::from_str::<LocalClientState>(&raw)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(st)
}

pub fn save_local_state(st: &LocalClientState) -> Result<PathBuf> {
    let path = local_state_path()?;
    let raw = serde_json::to_string_pretty(st)?;
    fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}
