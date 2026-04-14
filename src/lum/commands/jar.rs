use super::{CommandSpec, CoreContext};
use crate::lum::core::CoreApp;

// ==========================================
// CONFIGURACIÓN DE NOMBRES DE COMANDOS
// ==========================================
macro_rules! prefix { () => { "jar" } }
macro_rules! usage { ($rest:expr) => { concat!(prefix!(), " ", $rest) } }

const PREFIX: &str = prefix!();

const SUB_SETPATH: &str = "setpath";
const SUB_SETJAR: &str = "setjar";
const SUB_JVM:    &str = "jvm";
const SUB_ARGS:   &str = "args";
const SUB_START:  &str = "start";
const SUB_STOP:   &str = "stop";
// ==========================================

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: usage!("setpath <ruta>"),
        description: "Busca y guarda el .jar del servidor en una carpeta",
    },
    CommandSpec {
        usage: usage!("setjar <ruta.jar>"),
        description: "Guarda la ruta exacta de un archivo .jar",
    },
    CommandSpec {
        usage: usage!("jvm <args...>"),
        description: "Actualiza los argumentos de la JVM",
    },
    CommandSpec {
        usage: usage!("args <args...>"),
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
                println!("Uso: {} {} <ruta-a-carpeta-o-jar>", PREFIX, SUB_SETPATH);
                return true;
            }

            if ctx.server_runtime.is_some() {
                println!("[Core] Servidor activo. Apagándolo para cambiar ruta...");
                CoreApp::stop_server(ctx.server_runtime);
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            match CoreApp::set_server_path(ctx.server_cfg, &full_args) {
                Ok(msg) => {
                    println!("{msg}");
                    let _ = ctx.watchers_cfg.update_default_destination(&ctx.server_cfg.jar_path);
                    
                    if let Some(w_cfg) = ctx.watchers_cfg.watchers.get("default") {
                        ctx.watcher_manager.stop_named("default");
                        if w_cfg.enabled {
                            let _ = ctx.watcher_manager.start_named("default".to_string(), w_cfg.clone());
                        }
                    }
                }
                Err(e) => println!("[Core Error] {e}"),
            }
            true
        }

        s if s == SUB_SETJAR => {
            if full_args.trim().is_empty() {
                println!("Uso: {} {} <ruta-al-jar>", PREFIX, SUB_SETJAR);
                return true;
            }
            ctx.server_cfg.jar_path = full_args.trim().to_string();
            let _ = ctx.server_cfg.save();
            println!("[Core] JAR guardado: {}", ctx.server_cfg.jar_path);
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
            println!("Subcomandos de '{}' disponibles: {}, {}, {}, {}, {}, {}", 
                PREFIX, SUB_SETPATH, SUB_SETJAR, SUB_JVM, SUB_ARGS, SUB_START, SUB_STOP);
            true
        }
    }
}