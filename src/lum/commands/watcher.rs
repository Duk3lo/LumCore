use super::{CommandSpec, CoreContext};
use crate::lum::config::paths::{resolve, workspace_dir};
use crate::lum::config::watcher_config::WatcherConfig;

// ==========================================
// CONFIGURACIÓN DE NOMBRES DE COMANDOS
// ==========================================
macro_rules! prefix {
    () => {
        "watcher"
    };
}
macro_rules! usage {
    ($rest:expr) => {
        concat!(prefix!(), " ", $rest)
    };
}

const PREFIX: &str = prefix!();

const SUB_LIST: &str = "list";
const SUB_ADD: &str = "add";
const SUB_ENABLE: &str = "enable";
const SUB_DISABLE: &str = "disable";
const SUB_REMOVE: &str = "remove";
const SUB_SETDEST: &str = "setdest";
// ==========================================

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: usage!("list"),
        description: "Lista todos los watchers",
    },
    CommandSpec {
        usage: usage!("add <nombre> <source> [<destination>]"),
        description: "Crea un watcher nuevo",
    },
    CommandSpec {
        usage: usage!("enable <nombre>"),
        description: "Habilita un watcher",
    },
    CommandSpec {
        usage: usage!("disable <nombre>"),
        description: "Deshabilita un watcher",
    },
    CommandSpec {
        usage: usage!("remove <nombre>"),
        description: "Elimina un watcher",
    },
    CommandSpec {
        usage: usage!("setdest <nombre> <destination>"),
        description: "Cambia el destino de un watcher",
    },
];

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();

    let cmd_prefix = parts.next().unwrap_or("");
    if cmd_prefix != PREFIX {
        return false;
    }

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
        s if s == SUB_LIST => {
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

        s if s == SUB_ADD => {
            let args_vec: Vec<&str> = args.split_whitespace().collect();

            if args_vec.is_empty() {
                println!(
                    "Uso: {} {} <source>  O  {} {} <nombre> <source> [<destination>]",
                    PREFIX, SUB_ADD, PREFIX, SUB_ADD
                );
                return true;
            }

            let (name, source_raw, dest_raw) = match args_vec.len() {
                1 => {
                    let path = std::path::Path::new(args_vec[0]);
                    let auto_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unnamed_watcher");
                    (auto_name.to_string(), args_vec[0], "")
                }
                2 => {
                    (args_vec[0].to_string(), args_vec[1], "")
                }
                _ => {
                    (args_vec[0].to_string(), args_vec[1], args_vec[2])
                }
            };

            let source = match resolve(&workspace, source_raw) {
                Some(p) => p,
                None => {
                    println!("[Watcher Error] source inválido");
                    return true;
                }
            };

            let final_dest_raw = if dest_raw.is_empty() {
                crate::lum::config::paths::SYNC_MODS_DIR
            } else {
                dest_raw
            };

            let destination = match resolve(&workspace, final_dest_raw) {
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
            cfg.enabled = true;

            ctx.watchers_cfg.watchers.insert(name.clone(), cfg.clone());

            if let Err(e) = ctx.watchers_cfg.save() {
                println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                return true;
            }

            if let Err(e) = ctx.watcher_manager.start_named(name.clone(), cfg) {
                println!("[Watcher Error] No se pudo iniciar watcher: {e}");
            }

            println!(
                "[Watcher] agregado y activado: {} (Destino: {:?})",
                name,
                ctx.watchers_cfg
                    .watchers
                    .get(&name)
                    .unwrap()
                    .destination_path
            );
            true
        }
        s if s == SUB_ENABLE => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: {} {} <nombre>", PREFIX, SUB_ENABLE);
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

        s if s == SUB_DISABLE => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: {} {} <nombre>", PREFIX, SUB_DISABLE);
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

        s if s == SUB_REMOVE => {
            let name = args.trim();
            if name.is_empty() {
                println!("Uso: {} {} <nombre>", PREFIX, SUB_REMOVE);
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

        s if s == SUB_SETDEST => {
            let mut p = args.split_whitespace();
            let name = p.next().unwrap_or("");
            let dest_raw = p.next().unwrap_or("");

            if name.is_empty() || dest_raw.is_empty() {
                println!("Uso: {} {} <nombre> <destination>", PREFIX, SUB_SETDEST);
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
            println!(
                "Comandos disponibles: {}, {}, {}, {}, {}, {}",
                SUB_LIST, SUB_ADD, SUB_ENABLE, SUB_DISABLE, SUB_REMOVE, SUB_SETDEST
            );
            true
        }
    }
}
