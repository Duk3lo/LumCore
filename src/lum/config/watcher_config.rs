use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub enabled: bool,
    pub watch_subfolders: bool,
    pub multi_sync: bool,
    pub copy_on_init: bool,

    // rutas que el watcher vigila
    pub source_paths: Vec<PathBuf>,

    // ruta a donde se copia todo
    pub destination_path: Option<PathBuf>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_subfolders: true,
            multi_sync: false,
            copy_on_init: true,
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