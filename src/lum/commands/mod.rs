// src/lum/commands/mod.rs
pub mod core;
pub mod jar;
pub mod watcher;
pub mod curseforge;
pub mod github;  // <-- NUEVO

use crate::lum::config::jar_config::ServerConfig;
use crate::lum::config::watcher_config::WatchersConfig;
use crate::lum::config::curseforge_config::CurseForgeConfig;
use crate::lum::config::github_config::GitHubConfig;   
use crate::lum::config::updates_config::UpdatesConfig; 
use crate::lum::api::updater::UpdaterManager;          
use crate::lum::core_app::{CoreEvent, ServerRuntime};
use crate::lum::watchers::watcher_manager::WatcherManager;
use std::sync::mpsc;

pub struct CoreContext<'a> {
    pub server_cfg: &'a mut ServerConfig,
    pub watchers_cfg: &'a mut WatchersConfig,
    pub curseforge_cfg: &'a mut CurseForgeConfig,
    pub github_cfg: &'a mut GitHubConfig,        
    pub updates_cfg: &'a mut UpdatesConfig,      
    pub updater_manager: &'a mut UpdaterManager, 
    pub watcher_manager: &'a mut WatcherManager,
    pub server_runtime: &'a mut Option<ServerRuntime>,
    pub event_tx: &'a mpsc::Sender<CoreEvent>,
}

pub fn print_help() {
    println!("--- CORE COMMANDS (RUST EDITION) ---");
    println!(">> core status      - Muestra el estado general del sistema");
    println!(">> core updater     - Gestor del Auto-Actualizador (enable, disable, restart)");
    println!(">> jar <cmd>        - Manejo del servidor (start, stop, setjar, jvm)");
    println!(">> watcher <cmd>    - Sincronización de carpetas (add, remove, list)");
    println!(">> cf <cmd>         - Gestor de CurseForge (add, sync, remove, list)");
    println!(">> gh <cmd>         - Gestor de GitHub (add, sync, remove, list)");
    println!(">> exit / stop      - Apaga el servidor y cierra la consola");
    println!("-----------------------------------------");
}

pub fn dispatch(input: &str, ctx: &mut CoreContext) -> bool {
    if jar::handle(input, ctx) { return true; }
    if watcher::handle(input, ctx) { return true; }
    if curseforge::handle(input, ctx) { return true; }
    if github::handle(input, ctx) { return true; }  
    if core::handle(input, ctx) { return true; }

    false
}