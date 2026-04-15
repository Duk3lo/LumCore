use super::{CommandSpec, CoreContext};
use crate::lum::core::CoreApp;

// ==========================================
// CONFIGURACIÓN DE NOMBRES DE COMANDOS
// ==========================================
macro_rules! prefix {
    () => {
        "jar"
    };
}

macro_rules! usage {
    ($rest:expr) => {
        concat!(prefix!(), " ", $rest)
    };
}

const PREFIX: &str = prefix!();
const SUB_SETPATH: &str = "setpath";
const SUB_SETJAR: &str = "setjar";
const SUB_JVM: &str = "jvm";
const SUB_ARGS: &str = "args";
const SUB_START: &str = "start";
const SUB_STOP: &str = "stop";

// ==========================================
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: usage!("setpath "),
        description: "Busca y guarda el .jar del servidor en una carpeta",
    },
    CommandSpec {
        usage: usage!("setjar "),
        description: "Guarda la ruta exacta de un archivo .jar",
    },
    CommandSpec {
        usage: usage!("jvm "),
        description: "Actualiza los argumentos de la JVM",
    },
    CommandSpec {
        usage: usage!("args "),
        description: "Actualiza los argumentos del propio jar",
    },
    CommandSpec {
        usage: usage!("start"),
        description: "Inicia el servidor",
    },
    CommandSpec {
        usage: usage!("stop"),
        description: "Detiene el servidor",
    },
];

fn refresh_default_watcher(ctx: &mut CoreContext) {
    let _ = ctx.watchers_cfg.update_default_destination(&ctx.server_cfg.jar_path);

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

    // Validamos que el comando empiece por nuestro prefijo
    let cmd_prefix = parts.next().unwrap_or("");
    if cmd_prefix != PREFIX {
        return false;
    }

    let sub = parts.next().unwrap_or("").to_lowercase();
    let full_args = parts.collect::<Vec<_>>().join(" ").replace('"', "");

    match sub.as_str() {
        s if s == SUB_SETPATH => {
            if full_args.trim().is_empty() {
                println!("Uso: {} {} ", PREFIX, SUB_SETPATH);
                return true;
            }

            let was_running = ctx.server_runtime.is_some();

            if was_running {
                println!("[Core] Servidor activo.");
                println!("Apagándolo para cambiar ruta...");
                CoreApp::stop_server(ctx.server_runtime);
            }

            match CoreApp::set_server_path(ctx.server_cfg, &full_args) {
                Ok(msg) => {
                    println!("{msg}");
                    refresh_default_watcher(ctx);

                    if was_running {
                        if let Err(e) = CoreApp::start_server(ctx.server_cfg, ctx.server_runtime) {
                            println!("[Core Error] {e}");
                        }
                    }
                }
                Err(e) => println!("[Core Error] {e}"),
            }

            true
        }

        s if s == SUB_SETJAR => {
            if full_args.trim().is_empty() {
                println!("Uso: {} {} ", PREFIX, SUB_SETJAR);
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
                if let Err(e) = CoreApp::start_server(ctx.server_cfg, ctx.server_runtime) {
                    println!("[Core Error] {e}");
                }
            }

            true
        }

        s if s == SUB_JVM => {
            ctx.server_cfg.jvm_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] JVM args actualizados.");
            true
        }

        s if s == SUB_ARGS => {
            ctx.server_cfg.jar_args = CoreApp::parse_args(&full_args);
            let _ = ctx.server_cfg.save();
            println!("[Core] Jar args actualizados.");
            true
        }

        s if s == SUB_START => {
            if let Err(e) = CoreApp::start_server(ctx.server_cfg, ctx.server_runtime) {
                println!("[Core Error] {e}");
            }
            true
        }

        s if s == SUB_STOP => {
            CoreApp::stop_server(ctx.server_runtime);
            println!("[Core] Servidor detenido.");
            true
        }

        _ => {
            println!(
                "Subcomandos de '{}' disponibles: {}, {}, {}, {}, {}, {}",
                PREFIX, SUB_SETPATH, SUB_SETJAR, SUB_JVM, SUB_ARGS, SUB_START, SUB_STOP
            );
            true
        }
    }
}