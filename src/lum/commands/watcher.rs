use super::{CommandSpec, CoreContext};
use crate::lum::config::paths::{resolve, workspace_dir};
use crate::lum::config::watcher_config::WatcherConfig;

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: "core-watcher list",
        description: "Lista todos los watchers",
    },
    CommandSpec {
        usage: "core-watcher add <nombre> <source> <destination>",
        description: "Crea un watcher nuevo",
    },
    CommandSpec {
        usage: "core-watcher enable <nombre>",
        description: "Habilita un watcher",
    },
    CommandSpec {
        usage: "core-watcher disable <nombre>",
        description: "Deshabilita un watcher",
    },
    CommandSpec {
        usage: "core-watcher remove <nombre>",
        description: "Elimina un watcher",
    },
    CommandSpec {
        usage: "core-watcher setdest <nombre> <destination>",
        description: "Cambia el destino de un watcher",
    },
];

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();
    let _ = parts.next(); // core-watcher
    let sub = parts.next().unwrap_or("").to_lowercase();
    let args = parts.collect::<Vec<_>>().join(" ");

    let workspace = match workspace_dir() {
        Ok(p) => p,
        Err(e) => {
            println!("[Watcher Error] No se pudo resolver workspace: {e}");
            return true;
        }
    };

    match sub.as_str() {
        "list" => {
            println!("--- Watchers ---");
            if ctx.watchers_cfg.watchers.is_empty() {
                println!("(vacío)");
                return true;
            }

            for (name, w) in &ctx.watchers_cfg.watchers {
                println!(
                    "{} | enabled={} | multi_sync={} | copy_on_init={} | sources={} | dest={}",
                    name,
                    w.enabled,
                    w.multi_sync,
                    w.copy_on_init,
                    w.source_paths.len(),
                    w.destination_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "none".to_string())
                );
            }
            true
        }

        "add" => {
            let mut p = args.split_whitespace();
            let name = p.next().unwrap_or("");
            let source_raw = p.next().unwrap_or("");
            let dest_raw = p.next().unwrap_or("");

            if name.is_empty() || source_raw.is_empty() || dest_raw.is_empty() {
                println!("Uso: core-watcher add <nombre> <source> <destination>");
                return true;
            }

            let source = match resolve(&workspace, source_raw) {
                Some(p) => p,
                None => {
                    println!("[Watcher Error] source inválido");
                    return true;
                }
            };

            let destination = match resolve(&workspace, dest_raw) {
                Some(p) => p,
                None => {
                    println!("[Watcher Error] destination inválido");
                    return true;
                }
            };

            if !source.exists() {
                println!("[Watcher Error] La ruta de origen no existe: {:?}", source);
                return true;
            }

            let mut cfg = WatcherConfig::default();
            cfg.source_paths = vec![source];
            cfg.destination_path = Some(destination);

            ctx.watchers_cfg.watchers.insert(name.to_string(), cfg.clone());

            if let Err(e) = ctx.watchers_cfg.save() {
                println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                return true;
            }

            if cfg.enabled {
                if let Err(e) = ctx.watcher_manager.start_named(name.to_string(), cfg) {
                    println!("[Watcher Error] No se pudo iniciar watcher: {e}");
                }
            }

            println!("[Watcher] agregado: {}", name);
            true
        }

        "enable" => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: core-watcher enable <nombre>");
                return true;
            }

            let cloned = if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.enabled = true;
                Some(w.clone())
            } else {
                None
            };

            let Some(cfg) = cloned else {
                println!("[Watcher Error] No existe watcher: {}", name);
                return true;
            };

            if let Err(e) = ctx.watchers_cfg.save() {
                println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                return true;
            }

            ctx.watcher_manager.stop_named(name);
            if let Err(e) = ctx.watcher_manager.start_named(name.to_string(), cfg) {
                println!("[Watcher Error] No se pudo reiniciar watcher: {e}");
            } else {
                println!("[Watcher] habilitado: {}", name);
            }

            true
        }

        "disable" => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: core-watcher disable <nombre>");
                return true;
            }

            let exists = if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.enabled = false;
                true
            } else {
                false
            };

            if !exists {
                println!("[Watcher Error] No existe watcher: {}", name);
                return true;
            }

            if let Err(e) = ctx.watchers_cfg.save() {
                println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                return true;
            }

            ctx.watcher_manager.stop_named(name);
            println!("[Watcher] deshabilitado: {}", name);
            true
        }

        "remove" => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: core-watcher remove <nombre>");
                return true;
            }

            ctx.watcher_manager.stop_named(name);

            if ctx.watchers_cfg.watchers.remove(name).is_some() {
                if let Err(e) = ctx.watchers_cfg.save() {
                    println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                    return true;
                }
                println!("[Watcher] eliminado: {}", name);
            } else {
                println!("[Watcher Error] No existe watcher: {}", name);
            }

            true
        }

        "setdest" => {
            let mut p = args.split_whitespace();
            let name = p.next().unwrap_or("");
            let dest_raw = p.next().unwrap_or("");

            if name.is_empty() || dest_raw.is_empty() {
                println!("Uso: core-watcher setdest <nombre> <destination>");
                return true;
            }

            let destination = match resolve(&workspace, dest_raw) {
                Some(p) => p,
                None => {
                    println!("[Watcher Error] destination inválido");
                    return true;
                }
            };

            let cloned = if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.destination_path = Some(destination);
                Some(w.clone())
            } else {
                None
            };

            let Some(cfg) = cloned else {
                println!("[Watcher Error] No existe watcher: {}", name);
                return true;
            };

            if let Err(e) = ctx.watchers_cfg.save() {
                println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                return true;
            }

            ctx.watcher_manager.stop_named(name);
            if cfg.enabled {
                let _ = ctx.watcher_manager.start_named(name.to_string(), cfg);
            }

            println!("[Watcher] destino actualizado: {}", name);
            true
        }

        _ => {
            println!("Comandos: list, add, enable, disable, remove, setdest");
            true
        }
    }
}