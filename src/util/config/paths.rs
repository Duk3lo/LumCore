use std::env;
use std::path::PathBuf;

pub const CORE_CONFIG_DIR: &str = "core_config";
pub const SERVER_CONFIG_FILE: &str = "config.json";
pub const OTHER_CONFIG_FILE: &str = "other.json";

pub fn base_config_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("Could not get executable path: {e}"))?;

    let exe_dir = exe_path
        .parent()
        .ok_or("Executable has no parent directory".to_string())?;

    Ok(exe_dir.join(CORE_CONFIG_DIR))
}