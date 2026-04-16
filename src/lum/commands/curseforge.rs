use super::CoreContext;
use crate::lum::api::curseforge_api::CurseForgeClient;
use crate::lum::config::curseforge_config::CurseForgeResource;
use crate::lum::config::paths;

const PREFIX: &str = "cf";

fn sanitize_key(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
}

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();

    if parts.next() != Some(PREFIX) {
        return false;
    }

    let sub = parts.next().unwrap_or("help").to_lowercase();
    let arg = parts.collect::<Vec<_>>().join(" ");

    let client = CurseForgeClient::new(&ctx.curseforge_cfg.global_api_key);

    match sub.as_str() {
        "add" => {
            if arg.is_empty() {
                println!("[CurseForge] Uso: cf add <ID o Nombre del Mod>");
                return true;
            }
            if let Ok(id) = arg.parse::<u32>() {
                println!("[CurseForge] Buscando el mod con ID {}...", id);
                match client.get_mod_info(id) {
                    Ok(mod_info) => {
                        let key = sanitize_key(&mod_info.name);
                        if ctx.curseforge_cfg.resources.contains_key(&key) {
                            println!("[CurseForge] El mod '{}' ya está registrado.", key);
                        } else {
                            // AQUÍ ESTÁ EL CAMBIO IMPORTANTE: Destino 'syncmods'
                            let new_res = CurseForgeResource::new(id, paths::SYNC_MODS_DIR.to_string());
                            ctx.curseforge_cfg.resources.insert(key.clone(), new_res);
                            println!("[CurseForge] PROCEDER CON LA INSTALACIÓN: {} (ID: {})", mod_info.name, id);
                        }
                    }
                    Err(e) => println!("[Error CurseForge] No se encontró el mod: {}", e),
                }
            } else {
                println!("[CurseForge] Buscando coincidencias para '{}' en CurseForge...", arg);
                match client.search_mod(ctx.curseforge_cfg.global_game_id, &arg) {
                    Ok(results) => {
                        if results.is_empty() {
                            println!("[CurseForge] No se encontraron resultados para: {}", arg);
                        } else if results.len() == 1 {
                            println!("[CurseForge] Único resultado exacto. Añadiendo ID {}...", results[0].id);
                            let key = sanitize_key(&results[0].name);
                            // AQUÍ TAMBIÉN: Destino 'syncmods'
                            let new_res = CurseForgeResource::new(results[0].id, paths::SYNC_MODS_DIR.to_string());
                            ctx.curseforge_cfg.resources.insert(key, new_res);
                        } else {
                            println!("========================================");
                            println!("🔎 Se encontraron varios mods para: {}", arg);
                            println!("========================================");
                            for (i, res) in results.iter().take(5).enumerate() {
                                println!("{}. Nombre: {}   [ID: {}]", i + 1, res.name, res.id);
                            }
                            println!("----------------------------------------");
                            println!("👉 Para instalar uno, copia su ID y usa: cf add <ID>");
                            println!("========================================");
                        }
                    }
                    Err(e) => println!("[Error CurseForge] Error al buscar mod: {}", e),
                }
            }
        }

        "remove" => {
            if arg.is_empty() {
                println!("[CurseForge] Uso: cf remove <key>");
                return true;
            }

            if ctx.curseforge_cfg.resources.remove(&arg).is_some() {
                println!("[CurseForge] Mod '{}' eliminado de la lista.", arg);
                println!("[CurseForge] Nota: El archivo físico en el disco no ha sido eliminado.");
            } else {
                println!("[CurseForge] No se encontró ningún mod con la clave '{}'.", arg);
            }
        }

        "restore" => {
            if arg.is_empty() {
                println!("[CurseForge] Uso: cf restore <key>");
                return true;
            }

            if let Some(resource) = ctx.curseforge_cfg.resources.get_mut(&arg) {
                if let Err(e) = client.restore_latest_backup(resource, &arg) {
                    println!("[Error CurseForge] Fallo al restaurar backup: {}", e);
                }
            } else {
                println!("[CurseForge] No se encontró '{}' en tu configuración.", arg);
            }
        }

        "auto-search" => {
            let state = arg.trim();
            if state == "true" {
                ctx.curseforge_cfg.auto_search_untracked_mods = true;
                println!("[CurseForge] Búsqueda automática ACTIVADA");
            } else if state == "false" {
                ctx.curseforge_cfg.auto_search_untracked_mods = false;
                println!("[CurseForge] Búsqueda automática DESACTIVADA");
            } else {
                println!("[CurseForge] Uso: cf auto-search <true|false>");
            }
        }

        "ignore" => {
            let mut p = arg.split_whitespace();
            let action = p.next().unwrap_or("");
            let file = p.collect::<Vec<_>>().join(" ");

            match action {
                "list" => {
                    println!("--- Archivos Ignorados en CurseForge ---");
                    if ctx.curseforge_cfg.ignored_untracked_files.is_empty() {
                        println!("  (Ninguno)");
                    } else {
                        for f in &ctx.curseforge_cfg.ignored_untracked_files {
                            println!("  - {}", f);
                        }
                    }
                }
                "add" => {
                    if file.is_empty() {
                        println!("[CurseForge] Especifica el archivo: cf ignore add <archivo.jar>");
                    } else if !ctx.curseforge_cfg.ignored_untracked_files.contains(&file) {
                        ctx.curseforge_cfg.ignored_untracked_files.push(file.clone());
                        println!("[CurseForge] Archivo '{}' añadido a la lista de ignorados.", file);
                    } else {
                        println!("[CurseForge] El archivo ya estaba en la lista de ignorados.");
                    }
                }
                "remove" => {
                    if file.is_empty() {
                        println!("[CurseForge] Especifica el archivo: cf ignore remove <archivo.jar>");
                    } else {
                        let original_len = ctx.curseforge_cfg.ignored_untracked_files.len();
                        ctx.curseforge_cfg.ignored_untracked_files.retain(|x| x != &file);
                        if ctx.curseforge_cfg.ignored_untracked_files.len() < original_len {
                            println!("[CurseForge] Archivo '{}' removido de la lista de ignorados.", file);
                        } else {
                            println!("[CurseForge] El archivo no estaba en la lista de ignorados.");
                        }
                    }
                }
                _ => println!("[CurseForge] Uso: cf ignore <list|add|remove> [archivo.jar]"),
            }
        }

        "list" => {
            println!("--- Mods en CurseForge ---");
            if ctx.curseforge_cfg.resources.is_empty() {
                println!("  (No hay mods registrados)");
            } else {
                for (key, res) in &ctx.curseforge_cfg.resources {
                    let status = if res.enable { "✅ [ON]" } else { "❌ [OFF]" };
                    let file = res.local_file_name.as_deref().unwrap_or("Ninguno");
                    println!("{} {} | ID: {} | Archivo: {}", status, key, res.project_id, file);
                }
            }
        }

        "sync" => {
            if arg.is_empty() {
                println!("[CurseForge] Uso: cf sync <key>");
                return true;
            }

            if let Some(resource) = ctx.curseforge_cfg.resources.get_mut(&arg) {
                match client.download_and_replace(resource, &arg) {
                    Ok(true) => println!("[CurseForge] Sincronización exitosa."),
                    Ok(false) => {}
                    Err(e) => println!("[Error CurseForge] Fallo al sincronizar: {}", e),
                }
            } else {
                println!("[CurseForge] No se encontró '{}' en tu configuración.", arg);
            }
        }

        "sync-all" => {
            println!("[CurseForge] Sincronizando todos los mods activos...");
            
            if ctx.curseforge_cfg.auto_search_untracked_mods {
                println!("[CurseForge] (Nota: auto-search está activado. Añade mods no rastreados con 'cf add <nombre>' si ves advertencias en el log).");
            }

            for (key, resource) in ctx.curseforge_cfg.resources.iter_mut() {
                if !resource.enable { continue; }
                if let Err(e) = client.download_and_replace(resource, key) {
                    println!("[Error] Falló '{}': {}", key, e);
                }
            }
            println!("[CurseForge] Sincronización general completada.");
        }

        "help" | _ => {
            println!("--- COMANDOS DE CURSEFORGE ---");
            println!("cf add <id/nombre> - Añade un mod por ID o lo busca por nombre");
            println!("cf remove <key>    - Elimina un mod de la lista");
            println!("cf restore <key>   - Restaura la versión anterior de un mod");
            println!("cf ignore <args>   - Ignora archivos en la búsqueda automática (list, add, remove)");
            println!("cf auto-search     - Activa/Desactiva auto detección (cf auto-search true/false)");
            println!("cf list            - Muestra todos los mods configurados");
            println!("cf sync <key>      - Actualiza/Descarga un mod específico");
            println!("cf sync-all        - Actualiza todos los mods habilitados");
        }
    }

    true
}