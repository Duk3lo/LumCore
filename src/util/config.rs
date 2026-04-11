use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum ConfigLocation {
    Local,
    Global,
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
    fn base_dir(location: ConfigLocation) -> Result<PathBuf, String> {
        match location {
            ConfigLocation::Local => {
                let exe_path = env::current_exe()
                    .map_err(|e| format!("Could not get executable path: {e}"))?;

                let exe_dir = exe_path
                    .parent()
                    .ok_or("Executable has no parent directory".to_string())?;

                Ok(exe_dir.join("core_config"))
            }
            ConfigLocation::Global => {
                let home = env::var("HOME")
                    .map_err(|_| "HOME environment variable not found".to_string())?;

                Ok(PathBuf::from(home).join(".lumcoreserver"))
            }
        }
    }

    pub fn load_or_create(location: ConfigLocation) -> Result<Self, String> {
        let config_dir = Self::base_dir(location)?;
        let config_file_path = config_dir.join("config.json");

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
                jvm_args: vec![String::from("")],
                jar_args: vec![String::from("--assets"), String::from("../Assets.zip")],
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