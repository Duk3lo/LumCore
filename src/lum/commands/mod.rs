pub mod core;
pub mod jar;
pub mod watcher;
pub mod curseforge;
pub mod github;

use crate::lum::api::updater::UpdaterManager;
use crate::lum::config::curseforge_config::CurseForgeConfig;
use crate::lum::config::github_config::GitHubConfig;
use crate::lum::config::healing_config::HealingConfig;
use crate::lum::config::jar_config::ServerConfig;
use crate::lum::config::updates_config::UpdatesConfig;
use crate::lum::config::watcher_config::WatchersConfig;
use crate::lum::core_app::{CoreEvent, ServerRuntime};
use crate::lum::health::health_monitor::HealthMonitor;
use crate::lum::watchers::watcher_manager::WatcherManager;
use std::sync::mpsc;

pub struct CoreContext<'a> {
    pub server_cfg: &'a mut ServerConfig,
    pub watchers_cfg: &'a mut WatchersConfig,
    pub curseforge_cfg: &'a mut CurseForgeConfig,
    pub github_cfg: &'a mut GitHubConfig,
    pub updates_cfg: &'a mut UpdatesConfig,
    pub healing_cfg: &'a mut HealingConfig,
    pub updater_manager: &'a mut UpdaterManager,
    pub watcher_manager: &'a mut WatcherManager,
    pub health_monitor: &'a mut HealthMonitor,
    pub server_runtime: &'a mut Option<ServerRuntime>,
    pub event_tx: &'a mpsc::Sender<CoreEvent>,
    pub command_history: &'a Vec<String>,
}

impl<'a> CoreContext<'a> {
    pub fn reload_all(&mut self) {
        if let Err(e) = self.server_cfg.reload() {
            println!("[Config][Server] {e}");
        }
        if let Err(e) = self.watchers_cfg.reload() {
            println!("[Config][Watchers] {e}");
        }
        if let Err(e) = self.curseforge_cfg.reload() {
            println!("[Config][CurseForge] {e}");
        }
        if let Err(e) = self.github_cfg.reload() {
            println!("[Config][GitHub] {e}");
        }
        if let Err(e) = self.updates_cfg.reload() {
            println!("[Config][Updates] {e}");
        }
        if let Err(e) = self.healing_cfg.reload() {
            println!("[Config][Healing] {e}");
        }
    }
}

pub fn print_help() {
    println!("--- CORE COMMANDS (RUST EDITION) ---");
    println!(">> help            - Muestra este menú otra vez");
    println!(">> quit            - Cierra el programa");
    println!(">> core status     - Muestra el estado general del sistema");
    println!(">> core updater    - Gestor del Auto-Actualizador");
    println!(">> core healing    - Monitor de salud");
    println!(">> core history    - Muestra el historial de comandos");
    println!(">> jar <cmd>       - Manejo del servidor");
    println!(">> watcher <cmd>   - Sincronización de carpetas");
    println!(">> cf <cmd>        - Gestor de CurseForge");
    println!(">> gh <cmd>        - Gestor de GitHub");
    println!("-----------------------------------------");
}

pub fn dispatch(input: &str, ctx: &mut CoreContext) -> bool {
    let trimmed = input.trim().to_lowercase();

    if trimmed == "help" {
        print_help();
        return true;
    }

    if jar::handle(input, ctx) { return true; }
    if watcher::handle(input, ctx) { return true; }
    if curseforge::handle(input, ctx) { return true; }
    if github::handle(input, ctx) { return true; }
    if core::handle(input, ctx) { return true; }

    false
}