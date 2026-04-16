use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::paths::{base_config_dir, HEALING_CONFIG_FILE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingConfig {
    pub enable: bool,

    // Tiempos dinámicos: D (Días), H (Horas), M (Minutos), S (Segundos)
    pub initial_delay: String,
    pub check_interval: String,
    pub scheduled_restart: String,

    // Configuración de TPS
    pub min_tps_threshold: f64,
    pub max_strikes: u32,

    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            initial_delay: "30S".to_string(),
            check_interval: "2D".to_string(),
            scheduled_restart: "4D".to_string(),
            min_tps_threshold: 15.0,
            max_strikes: 3,
            config_file_path: PathBuf::new(),
        }
    }
}

impl HealingConfig {
    pub fn load_or_create() -> Result<Self, String> {
        let config_dir = base_config_dir()?;
        let config_file_path = config_dir.join(HEALING_CONFIG_FILE);

        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Could not create config directory: {e}"))?;

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("Could not read config: {e}"))?;

            let mut config: HealingConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid JSON: {e}"))?;

            config.config_file_path = config_file_path;
            Ok(config)
        } else {
            let mut config = HealingConfig::default();
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