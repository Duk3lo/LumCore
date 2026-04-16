use super::CoreContext;
use crate::lum::api::github_api::GitHubClient;
use crate::lum::config::github_config::RepositoryResource;

const PREFIX: &str = "gh";

pub fn handle(input: &str, ctx: &mut CoreContext) -> bool {
    let mut parts = input.split_whitespace();

    if parts.next() != Some(PREFIX) { return false; }
    
    let sub = parts.next().unwrap_or("help").to_lowercase();
    let arg = parts.collect::<Vec<_>>().join(" ");
    let client = GitHubClient::new(&ctx.github_cfg.personal_token);

    match sub.as_str() {
        "add" => {
            if !arg.contains('/') {
                println!("[GitHub] Uso correcto: gh add <usuario/repositorio>");
                return true;
            }
            let key = arg.split('/').last().unwrap_or(&arg).to_lowercase();
            if ctx.github_cfg.resources.contains_key(&key) {
                println!("[GitHub] El repositorio '{}' ya existe.", key);
            } else {
                let res = RepositoryResource::new(arg.clone(), "mods".to_string());
                ctx.github_cfg.resources.insert(key.clone(), res);
                println!("[GitHub] Registrado: {} como '{}'.", arg, key);
            }
        }
        "remove" => {
            if ctx.github_cfg.resources.remove(&arg).is_some() {
                println!("[GitHub] Repositorio '{}' eliminado.", arg);
            } else {
                println!("[GitHub] No se encontró el repositorio: {}", arg);
            }
        }
        "sync" => {
            if let Some(res) = ctx.github_cfg.resources.get_mut(&arg) {
                if let Err(e) = client.download_and_replace(res, &arg) {
                    println!("[GitHub Error] {}", e);
                }
            } else {
                println!("[GitHub] No se encontró '{}'", arg);
            }
        }
        "sync-all" => {
            println!("[GitHub] Sincronizando todos los repositorios activos...");
            for (key, res) in ctx.github_cfg.resources.iter_mut() {
                if !res.enable { continue; }
                let _ = client.download_and_replace(res, key);
            }
        }
        "restore" => {
            if let Some(res) = ctx.github_cfg.resources.get_mut(&arg) {
                let _ = client.restore_latest_backup(res, &arg);
            }
        }
        "help" | _ => {
            println!("--- COMANDOS DE GITHUB ---");
            println!("gh add <usuario/repo> - Añade un repositorio");
            println!("gh remove <key>       - Elimina un repositorio");
            println!("gh sync <key>         - Actualiza un repo específico");
            println!("gh sync-all           - Actualiza todos los repos");
            println!("gh restore <key>      - Restaura la versión anterior");
        }
    }
    true
}