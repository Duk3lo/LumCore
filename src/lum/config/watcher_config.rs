use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// Importamos lo necesario de módulos hermanos
use super::paths::{WATCHERS_CONFIG_FILE, ensure_base_hierarchy};
use super::jar_config::{ConfigLocation, ServerConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub enabled: bool,
    pub watch_subfolders: bool,
    pub multi_sync: bool,
    pub copy_on_init: bool,
    pub extensions: Vec<String>,
    pub restart_server_extensions: Vec<String>,
    pub source_paths: Vec<PathBuf>,
    pub destination_path: Option<PathBuf>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_subfolders: false,
            multi_sync: false,
            copy_on_init: true,
            extensions: vec!["jar".to_string()],
            restart_server_extensions: vec!["jar".to_string()],
            source_paths: vec![],
            destination_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchersConfig {
    pub watchers: HashMap<String, WatcherConfig>,

    #[serde(skip)]
    pub config_dir: PathBuf,
    #[serde(skip)]
    pub config_file_path: PathBuf,
}

impl WatchersConfig {
    pub fn load_or_create() -> Result<Self, String> {
        let (config_dir, syncmods) = ensure_base_hierarchy()?;
        let config_file_path = config_dir.join(WATCHERS_CONFIG_FILE);

        if config_file_path.exists() {
            let content = fs::read_to_string(&config_file_path)
                .map_err(|e| format!("No pude leer watchers.json: {e}"))?;

            let mut cfg: WatchersConfig = serde_json::from_str(&content)
                .map_err(|e| format!("JSON inválido en watchers.json: {e}"))?;

            cfg.config_dir = config_dir;
            cfg.config_file_path = config_file_path;
            Ok(cfg)
        } else {
            let mut watchers = HashMap::new();
            let mut default_watcher = WatcherConfig::default();

            default_watcher.source_paths.push(syncmods);
            let auto_dest =
                if let Ok(server_cfg) = ServerConfig::load_or_create(ConfigLocation::Local) {
                    let jar_path = PathBuf::from(server_cfg.jar_path);

                    if jar_path.as_os_str().is_empty() {
                        None
                    } else {
                        jar_path.parent().map(|p| p.join("mods"))
                    }
                } else {
                    None
                };

            default_watcher.destination_path = auto_dest;

            watchers.insert("default".to_string(), default_watcher);

            let cfg = WatchersConfig {
                watchers,
                config_dir,
                config_file_path,
            };
            cfg.save()?;
            Ok(cfg)
        }
    }
    pub fn update_default_destination(&mut self, jar_path: &str) -> Result<(), String> {
        if jar_path.is_empty() {
            return Ok(());
        }

        let path = PathBuf::from(jar_path);
        if let Some(parent) = path.parent() {
            let mods_folder = parent.join("mods");
            let mut changed = false;
            if let Some(w) = self.watchers.get_mut("default") {
                w.destination_path = Some(mods_folder);
                changed = true;
            }
            if changed {
                self.save()?;
                if let Some(w) = self.watchers.get("default") {
                    println!(
                        "[Watcher] Destino 'default' actualizado a: {:?}",
                        w.destination_path
                    );
                }
            }
        }
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.config_file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No pude crear el directorio de config: {e}"))?;
        }

        let text = serde_json::to_string_pretty(self)
            .map_err(|e| format!("No pude serializar watchers.json: {e}"))?;

        fs::write(&self.config_file_path, text)
            .map_err(|e| format!("No pude escribir watchers.json: {e}"))?;

        Ok(())
    }
}
