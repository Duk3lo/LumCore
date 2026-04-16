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
            let action = parts.next().unwrap_or("help").to_lowercase();
            let target = parts.next().unwrap_or("").to_lowercase();

            match action.as_str() {
                "enable" | "disable" => {
                    if target.is_empty() {
                        println!("[Updater] Uso: core updater {} <github|curseforge|server|all>", action);
                        return true;
                    }

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
                    println!("[Updater] Reiniciando tareas en segundo plano...");
                    ctx.updater_manager.stop();
                    ctx.updater_manager.start(ctx.updates_cfg.clone());
                    println!("[Updater] Schedulers aplicados correctamente.");
                }
                "stop" => {
                    ctx.updater_manager.stop();
                }
                "start" => {
                    ctx.updater_manager.start(ctx.updates_cfg.clone());
                }
                "help" | _ => {
                    println!("--- COMANDOS DE CORE UPDATER ---");
                    println!("core updater enable <target>  - Activa autoupdate (github, curseforge, server, all)");
                    println!("core updater disable <target> - Desactiva autoupdate");
                    println!("core updater restart          - Aplica los cambios y reinicia los hilos");
                    println!("core updater stop             - Detiene todos los chequeos automáticos");
                }
            }
        }

        "help" | _ => {
            super::print_help();
        }
    }

    true
}