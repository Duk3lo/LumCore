use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fs, path::PathBuf};

use super::paths::{base_config_dir, GITHUB_CONFIG_FILE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub personal_token: String,
    pub resources: HashMap<String, RepositoryResource>,

    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            personal_token: "".to_string(),
            resources: HashMap::new(),
            config_file_path: PathBuf::new(),
        }
    }
}

impl GitHubConfig {
    pub fn load_or_create() -> Result<Self, String> {
        let config_dir = base_config_dir()?;
        let config_file_path = config_dir.join(GITHUB_CONFIG_FILE);

        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Could not create config directory: {e}"))?;

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("Could not read config: {e}"))?;

            let mut config: GitHubConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON: {e}"))?;

            config.config_file_path = config_file_path;
            Ok(config)
        } else {
            let mut config = GitHubConfig::default();
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
pub struct RepositoryResource {
    pub enable: bool,
    pub repo_slug: String,
    pub custom_token: String,
    pub destination_path: String,
    pub keep_backup: bool,
    pub local_version_tag: String,
    pub local_file_name: Option<String>,
    pub verify_file_integrity: bool,
    pub last_verified_hash: String,
}

impl RepositoryResource {
    pub fn new(slug: String, destination_path: String) -> Self {
        Self {
            enable: true,
            repo_slug: slug,
            custom_token: "".to_string(),
            destination_path,
            keep_backup: true,
            local_version_tag: "".to_string(),
            local_file_name: None,
            verify_file_integrity: true,
            last_verified_hash: "".to_string(),
        }
    }
}