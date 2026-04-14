pub mod core;
pub mod jar;
pub mod watcher;

use crate::lum::config::jar_config::ServerConfig;
use crate::lum::config::watcher_config::WatchersConfig;
use crate::lum::core::ServerRuntime;
use crate::lum::watchers::watcher_manager::WatcherManager;

pub struct CoreContext<'a> {
    pub server_cfg: &'a mut ServerConfig,
    pub watchers_cfg: &'a mut WatchersConfig,
    pub watcher_manager: &'a mut WatcherManager,
    pub server_runtime: &'a mut Option<ServerRuntime>,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub usage: &'static str,
    pub description: &'static str,
}

pub fn print_help() {
    println!("--- CORE COMMANDS ---");
    for cmd in core::COMMANDS {
        println!("{:<36} - {}", cmd.usage, cmd.description);
    }

    println!("--- SERVER COMMANDS ---");
    for cmd in jar::COMMANDS {
        println!("{:<36} - {}", cmd.usage, cmd.description);
    }

    println!("--- WATCHER COMMANDS ---");
    for cmd in watcher::COMMANDS {
        println!("{:<36} - {}", cmd.usage, cmd.description);
    }

    println!("{:<36} - {}", "exit / stop", "Salir y apagar todo");
}

pub fn dispatch(input: &str, ctx: &mut CoreContext) -> bool {
    if jar::handle(input, ctx) {
        return true;
    }
    if watcher::handle(input, ctx) {
        return true;
    }
    if core::handle(input, ctx) {
        return true;
    }
    false
}