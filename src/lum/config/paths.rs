use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const MAIN_DIR: &str = "lumfolder";
pub const SYNC_MODS_DIR: &str = "syncmods";

pub const SERVER_CONFIG_FILE: &str = "config.json";
pub const WATCHERS_CONFIG_FILE: &str = "watchers.json";
pub const CURSEFORGE_CONFIG_FILE: &str = "curseforge_config.json";
pub const GITHUB_CONFIG_FILE: &str = "github_config.json";
pub const UPDATES_CONFIG_FILE: &str = "updates_config.json";

pub fn base_config_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("No pude obtener la ruta del ejecutable: {e}"))?;

    let exe_dir = exe_path
        .parent()
        .ok_or("El ejecutable no tiene directorio padre".to_string())?;

    Ok(exe_dir.join(MAIN_DIR))
}

pub fn workspace_dir() -> Result<PathBuf, String> {
    base_config_dir()
}

pub fn ensure_base_hierarchy() -> Result<(PathBuf, PathBuf), String> {
    let workspace = workspace_dir()?;
    let syncmods = workspace.join(SYNC_MODS_DIR);

    let cf_downloads = workspace.join("curseforge").join("downloads");
    let cf_backups = workspace.join("curseforge").join("backups");
    let gh_downloads = workspace.join("github").join("downloads");
    let gh_backups = workspace.join("github").join("backups");

    fs::create_dir_all(&workspace)
        .map_err(|e| format!("No pude crear el directorio base ({}): {}", MAIN_DIR, e))?;
    fs::create_dir_all(&syncmods)
        .map_err(|e| format!("No pude crear el directorio syncmods: {e}"))?;

    fs::create_dir_all(&cf_downloads)
        .map_err(|e| format!("No pude crear curseforge/downloads: {e}"))?;
    fs::create_dir_all(&cf_backups)
        .map_err(|e| format!("No pude crear curseforge/backups: {e}"))?;

    fs::create_dir_all(&gh_downloads)
        .map_err(|e| format!("No pude crear github/downloads: {e}"))?;
    fs::create_dir_all(&gh_backups)
        .map_err(|e| format!("No pude crear github/backups: {e}"))?;

    Ok((workspace, syncmods))
}

pub fn resolve(workspace: &Path, raw_path: &str) -> Option<PathBuf> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() {
        return None;
    }

    if raw_path.starts_with("./") || raw_path.starts_with(".\\") {
        return Some(workspace.join(&raw_path[2..]));
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(workspace.join(path))
    }
}