use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

pub const CORE_CONFIG_DIR: &str = "lumfolder";
pub const CORE_NEXUS_DIR: &str = "CoreNexus";
pub const SYNC_MODS_DIR: &str = "syncmods";

pub const SERVER_CONFIG_FILE: &str = "config.json";
pub const WATCHERS_CONFIG_FILE: &str = "watchers.json";

pub fn base_config_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("No pude obtener la ruta del ejecutable: {e}"))?;

    let exe_dir = exe_path
        .parent()
        .ok_or("El ejecutable no tiene directorio padre".to_string())?;

    Ok(exe_dir.join(CORE_CONFIG_DIR))
}

pub fn workspace_dir() -> Result<PathBuf, String> {
    Ok(base_config_dir()?.join(CORE_NEXUS_DIR))
}

pub fn syncmods_dir() -> Result<PathBuf, String> {
    Ok(workspace_dir()?.join(SYNC_MODS_DIR))
}

pub fn ensure_base_hierarchy() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let base_dir = base_config_dir()?;
    let workspace = base_dir.join(CORE_NEXUS_DIR);
    let syncmods = workspace.join(SYNC_MODS_DIR);

    fs::create_dir_all(&base_dir)
        .map_err(|e| format!("No pude crear lumfolder: {e}"))?;
    fs::create_dir_all(&workspace)
        .map_err(|e| format!("No pude crear CoreNexus: {e}"))?;
    fs::create_dir_all(&syncmods)
        .map_err(|e| format!("No pude crear syncmods: {e}"))?;

    Ok((base_dir, workspace, syncmods))
}

pub fn resolve(workspace: &Path, raw_path: &str) -> Option<PathBuf> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() {
        return None;
    }

    if raw_path.starts_with("./") || raw_path.starts_with(".\\") {
        return Some(workspace.join(&raw_path[2..]).to_path_buf());
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(workspace.join(path))
    }
}

pub fn relativize(workspace: &Path, absolute_path: &Path) -> String {
    if let Ok(relative) = absolute_path.strip_prefix(workspace) {
        format!("./{}", relative.to_string_lossy().replace("\\", "/"))
    } else {
        absolute_path.to_string_lossy().to_string()
    }
}