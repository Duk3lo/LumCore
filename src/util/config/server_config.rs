use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::paths::{base_config_dir, SERVER_CONFIG_FILE};

#[derive(Debug, Clone, Copy)]
pub enum ConfigLocation {
    Local,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    pub jar_path: String,
    pub jvm_args: Vec<String>,
    pub jar_args: Vec<String>,
    pub auto_restart: bool,

    #[serde(skip)]
    pub config_dir: PathBuf,
    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl ServerConfig {
    pub fn load_or_create(_location: ConfigLocation) -> Result<Self, String> {
        let config_dir = base_config_dir()?;
        let config_file_path = config_dir.join(SERVER_CONFIG_FILE);

        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Could not create config directory: {e}"))?;

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("Could not read config: {e}"))?;

            let mut config: ServerConfig =
                serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}"))?;

            config.config_dir = config_dir;
            config.config_file_path = config_file_path;

            Ok(config)
        } else {
            let config = ServerConfig {
                jar_path: String::from("/home/dukelo/Escritorio/Server/beat/Server/HytaleServer.jar"),
                jvm_args: vec![],
                jar_args: vec![
                    String::from("--assets"),
                    String::from("../Assets.zip"),
                ],
                auto_restart: true,
                config_dir,
                config_file_path,
            };

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