use super::CoreContext;

const PREFIX: &str = "core";

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();
    
    if parts.next() != Some(PREFIX) {
        return false; 
    }

    let sub = parts.next().unwrap_or("help").to_lowercase();

    match sub.as_str() {
        "status" => {
            println!("--- CORE STATUS ---");
            println!("Watchers registrados : {}", ctx.watchers_cfg.watchers.len());
            println!("Servidor activo      : {}", if ctx.server_runtime.is_some() { "SÍ" } else { "NO" });
            println!("Ruta del JAR         : {}", if ctx.server_cfg.jar_path.is_empty() { "No definida" } else { &ctx.server_cfg.jar_path });
        }

        "updater" => {
            // ... (Tu código de updater sin cambios) ...
            let action = parts.next().unwrap_or("help").to_lowercase();
            let target = parts.next().unwrap_or("").to_lowercase();

            match action.as_str() {
                "enable" | "disable" => {
                    let state = action == "enable";
                    let mut modified = false;

                    match target.as_str() {
                        "github" => { ctx.updates_cfg.github.enable = state; modified = true; }
                        "curseforge" => { ctx.updates_cfg.curseforge.enable = state; modified = true; }
                        "server" => { ctx.updates_cfg.server.enable_periodic_check = state; modified = true; }
                        "all" => {
                            ctx.updates_cfg.github.enable = state;
                            ctx.updates_cfg.curseforge.enable = state;
                            ctx.updates_cfg.server.enable_periodic_check = state;
                            modified = true;
                        }
                        _ => println!("[Updater] Objetivo desconocido: {}", target),
                    }

                    if modified {
                        println!("[Updater] {} ha sido {}D.", target.to_uppercase(), if state { "ACTIVA" } else { "DESACTIVA" });
                        println!("[Updater] Usa 'core updater restart' para aplicar los cambios en segundo plano.");
                    }
                }
                "restart" => {
                    ctx.updater_manager.stop();
                    ctx.updater_manager.start(ctx.updates_cfg.clone());
                    println!("[Updater] Reiniciando tareas...");
                }
                "stop" => ctx.updater_manager.stop(),
                "start" => ctx.updater_manager.start(ctx.updates_cfg.clone()),
                "help" | _ => println!("Comandos: enable/disable <target>, restart, stop, start")
            }
        }

        // --- NUEVO COMANDO HEALING ---
        "healing" => {
            let action = parts.next().unwrap_or("status").to_lowercase();

            match action.as_str() {
                "status" => {
                    let is_active = ctx.server_runtime.is_some();
                    ctx.health_monitor.print_health_status(is_active);
                }
                "enable" => {
                    ctx.healing_cfg.enable = true;
                    ctx.health_monitor.start(ctx.healing_cfg);
                    println!("[Core] Health monitor ACTIVADO.");
                }
                "disable" => {
                    ctx.healing_cfg.enable = false;
                    ctx.health_monitor.stop();
                    println!("[Core] Health monitor DESACTIVADO.");
                }
                "help" | _ => {
                    println!("--- COMANDOS DE HEALING ---");
                    println!("core healing status  - Muestra la salud de CPU/RAM y estado del monitor");
                    println!("core healing enable  - Activa autochequeo de TPS y tiempos");
                    println!("core healing disable - Desactiva autochequeo");
                }
            }
        }

        "help" | _ => {
            super::print_help();
        }
    }

    true
}