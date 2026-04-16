use super::CoreContext;
use crate::lum::core_app::CoreApp;

const PREFIX: &str = "jar";

fn refresh_default_watcher(ctx: &mut CoreContext) {
    let _ = ctx
        .watchers_cfg
        .update_default_destination(&ctx.server_cfg.jar_path);

    if let Some(w_cfg) = ctx.watchers_cfg.watchers.get("default").cloned() {
        ctx.watcher_manager.stop_named("default");
        if w_cfg.enabled {
            let _ = ctx
                .watcher_manager
                .start_named("default".to_string(), w_cfg, ctx.event_tx.clone());
        }
    }
}

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();

    if parts.next() != Some(PREFIX) {
        return false;
    }

    let sub = parts.next().unwrap_or("help").to_lowercase();
    let full_args = parts.collect::<Vec<_>>().join(" ").replace('"', "");

    match sub.as_str() {
        "setpath" => {
            if full_args.trim().is_empty() {
                println!("Uso: jar setpath <ruta>");
                return true;
            }

            let was_running = ctx.server_runtime.is_some();
            if was_running {
                println!("[Core] Servidor activo. Apagándolo para cambiar ruta...");
                CoreApp::stop_server(ctx.server_runtime);
            }

            match CoreApp::set_server_path(ctx.server_cfg, &full_args) {
                Ok(msg) => {
                    println!("{msg}");
                    refresh_default_watcher(ctx);

                    if was_running {
                        if let Err(e) = CoreApp::start_server(
                            ctx.server_cfg,
                            ctx.server_runtime,
                            ctx.event_tx.clone(),
                        ) {
                            println!("[Core Error] {e}");
                        } else {
                            ctx.health_monitor.notify_server_started();
                        }
                    }
                }
                Err(e) => println!("[Core Error] {e}"),
            }
        }

        "setjar" => {
            if full_args.trim().is_empty() {
                println!("Uso: jar setjar <archivo.jar>");
                return true;
            }

            let was_running = ctx.server_runtime.is_some();
            if was_running {
                CoreApp::stop_server(ctx.server_runtime);
            }

            let jar_path = full_args.trim();
            if !jar_path.to_lowercase().ends_with(".jar") {
                println!("[Core Error] La ruta debe terminar en .jar");
                return true;
            }

            ctx.server_cfg.jar_path = jar_path.to_string();
            if let Err(e) = ctx.server_cfg.save() {
                println!("[Core Error] No se pudo guardar config: {e}");
                return true;
            }

            println!("[Core] JAR guardado: {}", ctx.server_cfg.jar_path);
            refresh_default_watcher(ctx);

            if was_running {
                if let Err(e) = CoreApp::start_server(
                    ctx.server_cfg,
                    ctx.server_runtime,
                    ctx.event_tx.clone(),
                ) {
                    println!("[Core Error] {e}");
                } else {
                    ctx.health_monitor.notify_server_started();
                }
            }
        }

        "jvm" => {
            ctx.server_cfg.jvm_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] Argumentos JVM actualizados.");
        }

        "args" => {
            ctx.server_cfg.jar_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] Argumentos de JAR actualizados.");
        }

        "start" => {
            if let Err(e) = CoreApp::start_server(
                ctx.server_cfg,
                ctx.server_runtime,
                ctx.event_tx.clone(),
            ) {
                println!("[Core Error] {e}");
            } else {
                ctx.health_monitor.notify_server_started();
            }
        }

        "stop" => {
            CoreApp::stop_server(ctx.server_runtime);
            ctx.health_monitor.server_stopped();
            println!("[Core] Servidor detenido.");
        }

        "help" | _ => {
            println!("--- COMANDOS DE JAR ---");
            println!("jar start         - Inicia el servidor");
            println!("jar stop          - Detiene el servidor");
            println!("jar setpath <dir> - Busca y guarda el .jar en una carpeta");
            println!("jar setjar <jar>  - Define la ruta exacta al .jar");
            println!("jar jvm <args>    - Actualiza los argumentos de la JVM");
            println!("jar args <args>   - Actualiza los argumentos del propio jar");
        }
    }

    true
}