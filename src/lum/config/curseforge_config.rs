use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fs, path::PathBuf};

use super::paths::{base_config_dir, CURSEFORGE_CONFIG_FILE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurseForgeConfig {
    pub global_api_key: String,
    pub global_game_id: u32,
    pub auto_search_untracked_mods: bool,
    pub ignored_untracked_files: Vec<String>,
    pub resources: HashMap<String, CurseForgeResource>,

    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl Default for CurseForgeConfig {
    fn default() -> Self {
        Self {
            global_api_key: "INSERT_YOUR_KEY_HERE".to_string(),
            global_game_id: 70216,
            auto_search_untracked_mods: true,
            ignored_untracked_files: vec![],
            resources: HashMap::new(),
            config_file_path: PathBuf::new(),
        }
    }
}

impl CurseForgeConfig {
    pub fn load_or_create() -> Result<Self, String> {
        let config_dir = base_config_dir()?;
        let config_file_path = config_dir.join(CURSEFORGE_CONFIG_FILE);

        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Could not create config directory: {e}"))?;

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("Could not read config: {e}"))?;

            let mut config: CurseForgeConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON: {e}"))?;

            config.config_file_path = config_file_path;
            Ok(config)
        } else {
            let mut config = CurseForgeConfig::default();
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
pub struct CurseForgeResource {
    pub enable: bool,
    pub project_id: u32,
    pub destination_path: String,
    pub keep_backup: bool,
    pub local_file_id: u32,
    pub local_file_name: Option<String>,
    pub verify_file_integrity: bool,
}

impl CurseForgeResource {
    pub fn new(project_id: u32, destination_path: String) -> Self {
        Self {
            enable: true,
            project_id,
            destination_path,
            keep_backup: true,
            local_file_id: 0,
            local_file_name: None,
            verify_file_integrity: true,
        }
    }
}