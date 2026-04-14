use super::{CommandSpec, CoreContext};

// ==========================================
// CONFIGURACIÓN DE NOMBRES DE COMANDOS
// ==========================================
// 1. Definimos la macro del prefijo para este archivo
macro_rules! prefix { () => { "core" } }
// 2. Macro para autogenerar el "usage" combinando prefijo + subcomando
macro_rules! usage { ($rest:expr) => { concat!(prefix!(), " ", $rest) } }

const PREFIX: &str = prefix!();

const SUB_HELP:   &str = "help";
const SUB_STATUS: &str = "status";
// ==========================================

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: usage!("help"),
        description: "Muestra la ayuda detallada",
    },
    CommandSpec {
        usage: usage!("status"),
        description: "Muestra el estado general del sistema",
    },
];

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();
    
    // 1. Validamos si el primer token coincide con nuestro prefijo
    let cmd_prefix = parts.next().unwrap_or("");
    
    if cmd_prefix != PREFIX {
        return false; 
    }

    // 2. Obtenemos el subcomando
    let sub = parts.next().unwrap_or("").to_lowercase();

    match sub.as_str() {
        s if s == SUB_HELP => {
            super::print_help();
            true
        }

        s if s == SUB_STATUS => {
            println!("--- CORE STATUS ---");
            println!("Watchers registrados : {}", ctx.watchers_cfg.watchers.len());
            println!("Servidor activo      : {}", if ctx.server_runtime.is_some() { "SÍ" } else { "NO" });
            println!("Ruta del JAR         : {}", if ctx.server_cfg.jar_path.is_empty() { "No definida" } else { &ctx.server_cfg.jar_path });
            true
        }

        _ => {
            println!("Subcomandos de '{}' disponibles: {}, {}", PREFIX, SUB_HELP, SUB_STATUS);
            true 
        }
    }
}