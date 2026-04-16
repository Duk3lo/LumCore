use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::paths::{base_config_dir, UPDATES_CONFIG_FILE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatesConfig {
    pub curseforge: CurseForgeUpdate,
    pub github: GitHubUpdate,
    pub server: ServerUpdate,

    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            curseforge: CurseForgeUpdate { enable: false, check_interval: "12H".into() },
            github: GitHubUpdate { enable: false, check_interval: "1D".into() },
            server: ServerUpdate {
                enable_periodic_check: false,
                enable_console_listener: true,
                check_interval: "6H".into(),
                check_command: "update check".into(),
                trigger_update_found: "new version found:".into(),
                download_command: "update download".into(),
                trigger_download_complete: vec!["100%".into(), "to apply download use".into()],
                apply_command: "update apply --confirm".into(),
            },
            config_file_path: PathBuf::new(),
        }
    }
}

impl UpdatesConfig {
    pub fn load_or_create() -> Result<Self, String> {
        let config_dir = base_config_dir()?;
        let config_file_path = config_dir.join(UPDATES_CONFIG_FILE);

        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Could not create config directory: {e}"))?;

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("Could not read config: {e}"))?;

            let mut config: UpdatesConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON: {e}"))?;

            config.config_file_path = config_file_path;
            Ok(config)
        } else {
            let mut config = UpdatesConfig::default();
            config.config_file_path = config_file_path;
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.config_file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not ensure config directory exists: {e}"))?;
        }

        let json_content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON serialization failed: {e}"))?;

        fs::write(&self.config_file_path, json_content)
            .map_err(|e| format!("Failed to write config file: {e}"))?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurseForgeUpdate { pub enable: bool, pub check_interval: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUpdate { pub enable: bool, pub check_interval: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdate {
    pub enable_periodic_check: bool,
    pub enable_console_listener: bool,
    pub check_interval: String,
    pub check_command: String,
    pub trigger_update_found: String,
    pub download_command: String,
    pub trigger_download_complete: Vec<String>,
    pub apply_command: String,
}