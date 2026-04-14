use super::{CommandSpec, CoreContext};

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        usage: "core-help",
        description: "Muestra la ayuda",
    },
    CommandSpec {
        usage: "core-status",
        description: "Muestra el estado general",
    },
];

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    match input.split_whitespace().next().unwrap_or("").to_lowercase().as_str() {
        "core-help" => {
            super::print_help();
            true
        }
        "core-status" => {
            println!("--- STATUS ---");
            println!("Watchers registrados: {}", ctx.watchers_cfg.watchers.len());
            println!("Servidor activo: {}", ctx.server_runtime.is_some());
            println!("Jar: {}", ctx.server_cfg.jar_path);
            true
        }
        _ => false,
    }
}