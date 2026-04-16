use super::CoreContext;
use crate::lum::config::paths::{resolve, workspace_dir};
use crate::lum::config::watcher_config::WatcherConfig;

const PREFIX: &str = "watcher";

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();

    if parts.next() != Some(PREFIX) {
        return false;
    }

    let sub = parts.next().unwrap_or("help").to_lowercase();
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
                    "{} | enabled={} | dest={}",
                    name,
                    w.enabled,
                    w.destination_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "none".to_string())
                );
            }
        }

        "add" => {
            let args_vec: Vec<&str> = args.split_whitespace().collect();

            if args_vec.is_empty() {
                println!("Uso: watcher add <origen>  o  watcher add <nombre> <origen> [destino]");
                return true;
            }

            let (name, source_raw, dest_raw) = match args_vec.len() {
                1 => {
                    let path = std::path::Path::new(args_vec[0]);
                    let auto_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed_watcher");
                    (auto_name.to_string(), args_vec[0], "")
                }
                2 => (args_vec[0].to_string(), args_vec[1], ""),
                _ => (args_vec[0].to_string(), args_vec[1], args_vec[2]),
            };

            let source = match resolve(&workspace, source_raw) {
                Some(p) => p,
                None => { println!("[Watcher Error] source inválido"); return true; }
            };

            let final_dest_raw = if dest_raw.is_empty() { crate::lum::config::paths::SYNC_MODS_DIR } else { dest_raw };
            
            let destination = match resolve(&workspace, final_dest_raw) {
                Some(p) => p,
                None => { println!("[Watcher Error] destination inválido"); return true; }
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
            let _ = ctx.watchers_cfg.save();

            if let Err(e) = ctx.watcher_manager.start_named(name.clone(), cfg, ctx.event_tx.clone()) {
                println!("[Watcher Error] No se pudo iniciar watcher: {e}");
            }

            println!("[Watcher] agregado y activado: {}", name);
        }

        "enable" => {
            let name = args.trim();
            if name.is_empty() { println!("Uso: watcher enable <nombre>"); return true; }

            if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.enabled = true;
                let cfg_clone = w.clone();
                let _ = ctx.watchers_cfg.save();
                
                ctx.watcher_manager.stop_named(name);
                let _ = ctx.watcher_manager.start_named(name.to_string(), cfg_clone, ctx.event_tx.clone());
                println!("[Watcher] habilitado: {}", name);
            } else {
                println!("[Watcher Error] No existe watcher: {}", name);
            }
        }

        "disable" => {
            let name = args.trim();
            if name.is_empty() { println!("Uso: watcher disable <nombre>"); return true; }

            if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.enabled = false;
                let _ = ctx.watchers_cfg.save();
                ctx.watcher_manager.stop_named(name);
                println!("[Watcher] deshabilitado: {}", name);
            } else {
                println!("[Watcher Error] No existe watcher: {}", name);
            }
        }

        "remove" => {
            let name = args.trim();
            if name.is_empty() { println!("Uso: watcher remove <nombre>"); return true; }

            ctx.watcher_manager.stop_named(name);
            if ctx.watchers_cfg.watchers.remove(name).is_some() {
                let _ = ctx.watchers_cfg.save();
                println!("[Watcher] eliminado: {}", name);
            } else {
                println!("[Watcher Error] No existe watcher: {}", name);
            }
        }

        "setdest" => {
            let mut p = args.split_whitespace();
            let name = p.next().unwrap_or("");
            let dest_raw = p.next().unwrap_or("");

            if name.is_empty() || dest_raw.is_empty() {
                println!("Uso: watcher setdest <nombre> <destino>");
                return true;
            }

            let destination = match resolve(&workspace, dest_raw) {
                Some(p) => p,
                None => { println!("[Watcher Error] destination inválido"); return true; }
            };

            if let Some(w) = ctx.watchers_cfg.watchers.get_mut(name) {
                w.destination_path = Some(destination);
                let cfg_clone = w.clone();
                let _ = ctx.watchers_cfg.save();

                ctx.watcher_manager.stop_named(name);
                if cfg_clone.enabled {
                    let _ = ctx.watcher_manager.start_named(name.to_string(), cfg_clone, ctx.event_tx.clone());
                }
                println!("[Watcher] destino actualizado: {}", name);
            } else {
                println!("[Watcher Error] No existe watcher: {}", name);
            }
        }

        "help" | _ => {
            println!("--- COMANDOS DE WATCHER ---");
            println!("watcher list                    - Lista todos los watchers");
            println!("watcher add <ruta>              - Crea un watcher nuevo");
            println!("watcher enable <nombre>         - Habilita un watcher");
            println!("watcher disable <nombre>        - Deshabilita un watcher");
            println!("watcher remove <nombre>         - Elimina un watcher");
            println!("watcher setdest <nombre> <ruta> - Cambia el destino de un watcher");
        }
    }

    true
}