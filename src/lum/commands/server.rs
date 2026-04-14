use super::{CommandSpec, CoreContext};
use crate::lum::core::CoreApp;

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: "server-path <ruta>",
        description: "Busca y guarda el .jar del servidor",
    },
    CommandSpec {
        usage: "server-jar <ruta.jar>",
        description: "Guarda la ruta exacta del jar",
    },
    CommandSpec {
        usage: "server-jvm-args <args...>",
        description: "Actualiza argumentos JVM",
    },
    CommandSpec {
        usage: "server-jar-args <args...>",
        description: "Actualiza argumentos del jar",
    },
    CommandSpec {
        usage: "start-server",
        description: "Inicia el servidor",
    },
    CommandSpec {
        usage: "stop-server",
        description: "Detiene el servidor",
    },
];

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();
    let command = parts.next().unwrap_or("").to_lowercase();
    let full_args = parts.collect::<Vec<_>>().join(" ").replace('"', "");

    match command.as_str() {
        "server-path" => {
            if full_args.trim().is_empty() {
                println!("Uso: server-path <ruta-a-carpeta-o-jar>");
                return true;
            }

            if ctx.server_runtime.is_some() {
                println!("[Core] Servidor activo detectado. Apagándolo antes de cambiar la ruta...");
                CoreApp::stop_server(ctx.server_runtime);
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            match CoreApp::set_server_path(ctx.server_cfg, &full_args) {
                Ok(msg) => {
                    println!("{msg}");

                    if let Err(e) = ctx
                        .watchers_cfg
                        .update_default_destination(&ctx.server_cfg.jar_path)
                    {
                        println!("[Watcher Warning] No se pudo actualizar el destino automático: {e}");
                    }

                    if let Some(w_cfg) = ctx.watchers_cfg.watchers.get("default") {
                        ctx.watcher_manager.stop_named("default");
                        if w_cfg.enabled {
                            let _ = ctx
                                .watcher_manager
                                .start_named("default".to_string(), w_cfg.clone());
                        }
                    }
                }
                Err(e) => println!("[Core Error] {e}"),
            }

            true
        }

        "server-jar" => {
            if full_args.trim().is_empty() {
                println!("Uso: server-jar <ruta-al-jar>");
                return true;
            }

            ctx.server_cfg.jar_path = full_args.trim().to_string();
            let _ = ctx.server_cfg.save();
            println!("[Core] JAR guardado: {}", ctx.server_cfg.jar_path);
            true
        }

        "server-jvm-args" => {
            ctx.server_cfg.jvm_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] JVM args actualizados.");
            true
        }

        "server-jar-args" => {
            ctx.server_cfg.jar_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] Jar args actualizados.");
            true
        }

        "start-server" => {
            if let Err(e) = CoreApp::start_server(ctx.server_cfg, ctx.server_runtime) {
                println!("[Core Error] {e}");
            }
            true
        }

        "stop-server" => {
            CoreApp::stop_server(ctx.server_runtime);
            println!("[Core] Servidor detenido.");
            true
        }

        _ => false,
    }
}