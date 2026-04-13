use super::config::server_config::{ConfigLocation, ServerConfig};
use crate::lum::config::paths::{resolve, workspace_dir};
use crate::lum::config::watcher_config::{WatcherConfig, WatchersConfig};
use crate::lum::java_jar_runner::JavaJarRunner;
use crate::lum::watchers::watcher_manager::WatcherManager;

use std::{
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

pub struct CoreApp;

struct ServerRuntime {
    tx: mpsc::Sender<String>,
    handle: thread::JoinHandle<()>,
}

impl CoreApp {
    pub fn start() {
        println!("--- Starting CoreNexus (Rust Edition) ---");

        let mut server_cfg = match ServerConfig::load_or_create(ConfigLocation::Local) {
            Ok(cfg) => cfg,
            Err(e) => {
                println!("[Core Error] Failed to load server config: {}", e);
                return;
            }
        };

        let mut watchers_cfg = match WatchersConfig::load_or_create() {
            Ok(cfg) => cfg,
            Err(e) => {
                println!("[Core Error] Failed to load watchers config: {}", e);
                return;
            }
        };

        let mut watcher_manager = WatcherManager::new();
        if let Err(e) = watcher_manager.start_all(&watchers_cfg) {
            println!("[Core Warning] Some watchers could not start: {}", e);
        }

        let mut server_runtime: Option<ServerRuntime> = None;

        println!("[Core] Ready. Type commands to send to the server (type 'exit' to quit).");
        println!("[Core] Commands:");
        println!("  server-path <ruta>");
        println!("  server-jar <ruta.jar>");
        println!("  server-jvm-args <args...>");
        println!("  server-jar-args <args...>");
        println!("  start-server");
        println!("  stop-server");
        println!("  core-watcher list | add | enable | disable | remove | setdest");
        println!("  core-help");
        println!("  core-status");

        let stdin = io::stdin();

        for line in stdin.lock().lines() {
            let input = match line {
                Ok(v) => v,
                Err(e) => {
                    println!("[Core Error] stdin error: {e}");
                    break;
                }
            };

            let cmd = input.trim();
            if cmd.is_empty() {
                continue;
            }

            if cmd == "exit" || cmd == "stop" {
                println!("[Core] Shutting down...");
                Self::stop_server(&mut server_runtime);
                break;
            }

            if Self::is_internal_command(cmd) {
                Self::handle_core_command(
                    cmd,
                    &mut server_cfg,
                    &mut watchers_cfg,
                    &mut watcher_manager,
                    &mut server_runtime,
                );
            } else {
                if let Some(runtime) = &server_runtime {
                    let _ = runtime.tx.send(cmd.to_string());
                } else {
                    println!("[Core] No hay servidor activo. Usa 'start-server' primero.");
                }
            }
        }

        println!("[Core] Waiting shutdown...");
        Self::stop_server(&mut server_runtime);
        watcher_manager.stop_all();

        println!("--- Everything is safely shut down. Goodbye! ---");
    }

    fn is_internal_command(input: &str) -> bool {
        let cmd = input.split_whitespace().next().unwrap_or("").to_lowercase();
        matches!(
            cmd.as_str(),
            "core-watcher"
                | "core-status"
                | "core-help"
                | "server-path"
                | "server-jar"
                | "server-jvm-args"
                | "server-jar-args"
                | "start-server"
                | "stop-server"
        )
    }

    fn handle_core_command(
        input: &str,
        server_cfg: &mut ServerConfig,
        watchers_cfg: &mut WatchersConfig,
        watcher_manager: &mut WatcherManager,
        server_runtime: &mut Option<ServerRuntime>,
    ) {
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or("").to_lowercase();

        // Obtenemos TODO lo que sigue después del comando como una sola cadena
        // y quitamos las comillas innecesarias
        let full_args = parts.collect::<Vec<_>>().join(" ").replace("\"", "");

        match command.as_str() {
            "core-watcher" => {
                // Este es el único que usa sub-comando, así que lo separamos aquí
                let mut sub_parts = full_args.split_whitespace();
                let sub = sub_parts.next().unwrap_or("").to_lowercase();
                let rest = sub_parts.collect::<Vec<_>>().join(" ");
                Self::handle_watcher_command(&sub, &rest, watchers_cfg, watcher_manager);
            }

            "core-status" => {
                println!("--- STATUS ---");
                println!("Watchers registrados: {}", watchers_cfg.watchers.len());
                println!("Servidor activo: {}", server_runtime.is_some());
                println!("Jar: {}", server_cfg.jar_path);
            }

            "core-help" => Self::print_help(),

            "server-path" => {
                if full_args.trim().is_empty() {
                    println!("Uso: server-path <ruta-a-carpeta-o-jar>");
                    return;
                }

                // 1. SI HAY UN SERVIDOR ENCENDIDO, LO APAGAMOS
                if server_runtime.is_some() {
                    println!(
                        "[Core] Servidor activo detectado. Enviando 'stop' antes de cambiar de ruta..."
                    );
                    Self::stop_server(server_runtime);
                    // Pequeña espera para que el proceso libere los archivos
                    thread::sleep(std::time::Duration::from_millis(500));
                }

                // 2. CAMBIAMOS LA RUTA
                match Self::set_server_path(server_cfg, &full_args) {
                    Ok(msg) => {
                        println!("{msg}");

                        // 3. ACTUALIZAMOS AUTOMÁTICAMENTE EL WATCHER DEFAULT
                        if let Err(e) =
                            watchers_cfg.update_default_destination(&server_cfg.jar_path)
                        {
                            println!(
                                "[Watcher Warning] No se pudo actualizar el destino automático: {e}"
                            );
                        }

                        // Opcional: Reiniciar el watcher en el manager para que use la nueva ruta
                        if let Some(w_cfg) = watchers_cfg.watchers.get("default") {
                            watcher_manager.stop_named("default");
                            if w_cfg.enabled {
                                let _ = watcher_manager
                                    .start_named("default".to_string(), w_cfg.clone());
                            }
                        }
                    }
                    Err(e) => println!("[Core Error] {e}"),
                }
            }

            "server-jar" => {
                if full_args.trim().is_empty() {
                    println!("Uso: server-jar <ruta-al-jar>");
                    return;
                }
                server_cfg.jar_path = full_args.trim().to_string();
                let _ = server_cfg.save();
                println!("[Core] JAR guardado: {}", server_cfg.jar_path);
            }

            "server-jvm-args" => {
                server_cfg.jvm_args = Self::parse_args(&full_args);
                let _ = server_cfg.save();
                println!("[Core] JVM args actualizados.");
            }

            "server-jar-args" => {
                server_cfg.jar_args = Self::parse_args(&full_args);
                let _ = server_cfg.save();
                println!("[Core] Jar args actualizados.");
            }

            "start-server" => {
                if let Err(e) = Self::start_server(server_cfg, server_runtime) {
                    println!("[Core Error] {e}");
                }
            }

            "stop-server" => {
                Self::stop_server(server_runtime);
                println!("[Core] Servidor detenido.");
            }

            _ => println!("[Core] Comando interno desconocido: {}", command),
        }
    }

    fn handle_watcher_command(
        sub: &str,
        args: &str,
        watchers_cfg: &mut WatchersConfig,
        watcher_manager: &mut WatcherManager,
    ) {
        let workspace = match workspace_dir() {
            Ok(p) => p,
            Err(e) => {
                println!("[Watcher Error] No se pudo resolver workspace: {e}");
                return;
            }
        };

        match sub {
            "list" => {
                println!("--- Watchers ---");
                if watchers_cfg.watchers.is_empty() {
                    println!("(vacío)");
                    return;
                }

                for (name, w) in &watchers_cfg.watchers {
                    println!(
                        "{} | enabled={} | multi_sync={} | copy_on_init={} | sources={} | dest={}",
                        name,
                        w.enabled,
                        w.multi_sync,
                        w.copy_on_init,
                        w.source_paths.len(),
                        w.destination_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "none".to_string())
                    );
                }
            }

            "add" => {
                let mut p = args.split_whitespace();
                let name = p.next().unwrap_or("");
                let source_raw = p.next().unwrap_or("");
                let dest_raw = p.next().unwrap_or("");

                if name.is_empty() || source_raw.is_empty() || dest_raw.is_empty() {
                    println!("Uso: core-watcher add <nombre> <source> <destination>");
                    return;
                }

                let source = match resolve(&workspace, source_raw) {
                    Some(p) => p,
                    None => {
                        println!("[Watcher Error] source inválido");
                        return;
                    }
                };

                let destination = match resolve(&workspace, dest_raw) {
                    Some(p) => p,
                    None => {
                        println!("[Watcher Error] destination inválido");
                        return;
                    }
                };

                if !source.exists() {
                    println!("[Watcher Error] La ruta de origen no existe: {:?}", source);
                    return;
                }

                let mut cfg = WatcherConfig::default();
                cfg.source_paths = vec![source];
                cfg.destination_path = Some(destination);

                watchers_cfg.watchers.insert(name.to_string(), cfg.clone());

                if let Err(e) = watchers_cfg.save() {
                    println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                    return;
                }

                if cfg.enabled {
                    if let Err(e) = watcher_manager.start_named(name.to_string(), cfg) {
                        println!("[Watcher Error] No se pudo iniciar watcher: {e}");
                    }
                }

                println!("[Watcher] agregado: {}", name);
            }

            "enable" => {
                let name = args.trim();
                if name.is_empty() {
                    println!("Uso: core-watcher enable <nombre>");
                    return;
                }

                let cloned = if let Some(w) = watchers_cfg.watchers.get_mut(name) {
                    w.enabled = true;
                    Some(w.clone())
                } else {
                    None
                };

                let Some(cfg) = cloned else {
                    println!("[Watcher Error] No existe watcher: {}", name);
                    return;
                };

                if let Err(e) = watchers_cfg.save() {
                    println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                    return;
                }

                watcher_manager.stop_named(name);
                if let Err(e) = watcher_manager.start_named(name.to_string(), cfg) {
                    println!("[Watcher Error] No se pudo reiniciar watcher: {e}");
                } else {
                    println!("[Watcher] habilitado: {}", name);
                }
            }

            "disable" => {
                let name = args.trim();
                if name.is_empty() {
                    println!("Uso: core-watcher disable <nombre>");
                    return;
                }

                let exists = if let Some(w) = watchers_cfg.watchers.get_mut(name) {
                    w.enabled = false;
                    true
                } else {
                    false
                };

                if !exists {
                    println!("[Watcher Error] No existe watcher: {}", name);
                    return;
                }

                if let Err(e) = watchers_cfg.save() {
                    println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                    return;
                }

                watcher_manager.stop_named(name);
                println!("[Watcher] deshabilitado: {}", name);
            }

            "remove" => {
                let name = args.trim();
                if name.is_empty() {
                    println!("Uso: core-watcher remove <nombre>");
                    return;
                }

                watcher_manager.stop_named(name);

                if watchers_cfg.watchers.remove(name).is_some() {
                    if let Err(e) = watchers_cfg.save() {
                        println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                        return;
                    }
                    println!("[Watcher] eliminado: {}", name);
                } else {
                    println!("[Watcher Error] No existe watcher: {}", name);
                }
            }

            "setdest" => {
                let mut p = args.split_whitespace();
                let name = p.next().unwrap_or("");
                let dest_raw = p.next().unwrap_or("");

                if name.is_empty() || dest_raw.is_empty() {
                    println!("Uso: core-watcher setdest <nombre> <destination>");
                    return;
                }

                let destination = match resolve(&workspace, dest_raw) {
                    Some(p) => p,
                    None => {
                        println!("[Watcher Error] destination inválido");
                        return;
                    }
                };

                let cloned = if let Some(w) = watchers_cfg.watchers.get_mut(name) {
                    w.destination_path = Some(destination);
                    Some(w.clone())
                } else {
                    None
                };

                let Some(cfg) = cloned else {
                    println!("[Watcher Error] No existe watcher: {}", name);
                    return;
                };

                if let Err(e) = watchers_cfg.save() {
                    println!("[Watcher Error] No se pudo guardar watchers.json: {e}");
                    return;
                }

                watcher_manager.stop_named(name);
                if cfg.enabled {
                    let _ = watcher_manager.start_named(name.to_string(), cfg);
                }

                println!("[Watcher] destino actualizado: {}", name);
            }

            _ => {
                println!("Comandos: list, add, enable, disable, remove, setdest");
            }
        }
    }

    fn parse_args(rest: &str) -> Vec<String> {
        rest.split_whitespace().map(|s| s.to_string()).collect()
    }

    fn set_server_path(server_cfg: &mut ServerConfig, raw: &str) -> Result<String, String> {
        let path = Self::resolve_native_path(raw)?;
        let jar_path = if path.is_dir() {
            Self::detect_jar_in_dir(&path)
                .ok_or_else(|| format!("No se encontró ningún .jar en {:?}", path))?
        } else {
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jar"))
                .unwrap_or(false)
            {
                path
            } else {
                return Err("La ruta debe ser una carpeta o un archivo .jar".to_string());
            }
        };

        server_cfg.jar_path = jar_path.to_string_lossy().to_string();
        server_cfg
            .save()
            .map_err(|e| format!("No se pudo guardar config: {e}"))?;

        Ok(format!(
            "[Core] JAR detectado y guardado: {}",
            server_cfg.jar_path
        ))
    }

    fn resolve_native_path(raw: &str) -> Result<PathBuf, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("Ruta vacía".to_string());
        }

        let path = PathBuf::from(raw);
        if path.is_absolute() {
            Ok(path)
        } else {
            std::env::current_dir()
                .map_err(|e| format!("No se pudo leer el directorio actual: {e}"))
                .map(|cwd| cwd.join(path))
        }
    }

    fn detect_jar_in_dir(dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;

        for entry in entries.flatten() {
            let path = entry.path();

            let is_jar = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jar"))
                .unwrap_or(false);

            if !is_jar {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            if name.contains("corenexus") {
                continue;
            }

            return Some(path);
        }

        None
    }

    fn start_server(
        server_cfg: &ServerConfig,
        server_runtime: &mut Option<ServerRuntime>,
    ) -> Result<(), String> {
        if server_runtime.is_some() {
            return Err("El servidor ya está en ejecución".to_string());
        }

        let runner = JavaJarRunner::from_config(server_cfg)?;
        let (tx, rx) = mpsc::channel::<String>();

        let handle = thread::spawn(move || {
            println!("[Core] Launching background thread for JAR...");
            runner.start_and_read(rx);
        });

        *server_runtime = Some(ServerRuntime { tx, handle });
        println!("[Core] Servidor iniciado.");
        Ok(())
    }

    fn stop_server(server_runtime: &mut Option<ServerRuntime>) {
        if let Some(runtime) = server_runtime.take() {
            let _ = runtime.tx.send("stop".to_string());
            let _ = runtime.handle.join();
        }
    }

    fn print_help() {
        println!("--- CORE COMMANDS ---");
        println!("server-path <ruta>");
        println!("server-jar <ruta.jar>");
        println!("server-jvm-args <args...>");
        println!("server-jar-args <args...>");
        println!("start-server");
        println!("stop-server");
        println!("core-watcher list");
        println!("core-watcher add <nombre> <source> <destination>");
        println!("core-watcher enable <nombre>");
        println!("core-watcher disable <nombre>");
        println!("core-watcher remove <nombre>");
        println!("core-watcher setdest <nombre> <destination>");
        println!("core-status");
        println!("exit / stop");
    }
}
