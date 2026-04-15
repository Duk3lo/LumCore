use crate::lum::commands::{self, CoreContext};
use crate::lum::config::jar_config::{ConfigLocation, ServerConfig};
use crate::lum::config::watcher_config::WatchersConfig;
use crate::lum::java_jar_runner::{JavaJarRunner, RunnerCommand};
use crate::lum::watchers::watcher_manager::WatcherManager;
use std::{
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub struct CoreApp;

pub struct ServerRuntime {
    pub tx: mpsc::Sender<RunnerCommand>,
    pub handle: thread::JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub enum CoreEvent {
    UserCommand(String),
    RestartRequested { changed_path: PathBuf },
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

        let (core_tx, core_rx) = mpsc::channel::<CoreEvent>();

        let stdin_tx = core_tx.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(value) => {
                        if stdin_tx.send(CoreEvent::UserCommand(value)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[Core Error] stdin error: {e}");
                        break;
                    }
                }
            }
        });

        if let Err(e) = watcher_manager.start_all(&watchers_cfg, core_tx.clone()) {
            println!("[Core Warning] Some watchers could not start: {}", e);
        }

        let mut server_runtime: Option<ServerRuntime> = None;
        let mut last_restart = Instant::now() - Duration::from_secs(10);

        println!("[Core] Ready.");
        println!("Type commands to send to the server (type 'exit' to quit).");
        println!("[Core] Commands:");
        commands::print_help();

        loop {
            let event = match core_rx.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            };

            match event {
                CoreEvent::UserCommand(input) => {
                    let cmd = input.trim();
                    if cmd.is_empty() {
                        continue;
                    }

                    if cmd == "exit" || cmd == "stop" {
                        println!("[Core] Shutting down...");
                        Self::stop_server(&mut server_runtime);
                        watcher_manager.stop_all();
                        break;
                    }

                    let mut ctx = CoreContext {
                        server_cfg: &mut server_cfg,
                        watchers_cfg: &mut watchers_cfg,
                        watcher_manager: &mut watcher_manager,
                        server_runtime: &mut server_runtime,
                        event_tx: &core_tx,
                    };

                    if commands::dispatch(cmd, &mut ctx) {
                        continue;
                    }

                    if let Some(runtime) = ctx.server_runtime.as_ref() {
                        let _ = runtime
                            .tx
                            .send(RunnerCommand::Input(cmd.to_string()));
                    } else {
                        println!("[Core] No hay servidor activo.");
                        println!("Usa 'jar start' primero.");
                    }
                }

                CoreEvent::RestartRequested { changed_path } => {
                    if last_restart.elapsed() < Duration::from_millis(1200) {
                        println!(
                            "[Watcher] Reinicio omitido por rebote: {:?}",
                            changed_path
                        );
                        continue;
                    }

                    last_restart = Instant::now();
                    println!(
                        "[Watcher] Cambio listo, reiniciando por {:?}...",
                        changed_path
                    );

                    let was_running = server_runtime.is_some();
                    if was_running {
                        Self::stop_server(&mut server_runtime);
                    }

                    if server_cfg.auto_restart || was_running {
                        if let Err(e) = Self::start_server(&server_cfg, &mut server_runtime) {
                            println!("[Core Error] No se pudo reiniciar el servidor: {e}");
                        }
                    }
                }
            }
        }

        println!("[Core] Waiting shutdown...");
        Self::stop_server(&mut server_runtime);
        watcher_manager.stop_all();
        println!("--- Everything is safely shut down.");
        println!("Goodbye! ---");
    }

    pub(crate) fn parse_args(rest: &str) -> Vec<String> {
        rest.split_whitespace().map(|s| s.to_string()).collect()
    }

    pub(crate) fn set_server_path(
        server_cfg: &mut ServerConfig,
        raw: &str,
    ) -> Result<String, String> {
        let path = Self::resolve_native_path(raw)?;

        let jar_path = if path.is_dir() {
            Self::detect_jar_in_dir(&path)
                .ok_or_else(|| format!("No se encontró ningún .jar en {:?}", path))?
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("jar"))
            .unwrap_or(false)
        {
            path
        } else {
            return Err("La ruta debe ser una carpeta o un archivo .jar".to_string());
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

            if name.contains(crate::lum::config::paths::MAIN_DIR) {
                continue;
            }

            return Some(path);
        }

        None
    }

    pub(crate) fn start_server(
        server_cfg: &ServerConfig,
        server_runtime: &mut Option<ServerRuntime>,
    ) -> Result<(), String> {
        if server_runtime.is_some() {
            return Err("El servidor ya está en ejecución".to_string());
        }

        let runner = JavaJarRunner::from_config(server_cfg)?;
        let (tx, rx) = mpsc::channel::<RunnerCommand>();

        let handle = thread::spawn(move || {
            println!("[Core] Launching background thread for JAR...");
            runner.start_and_read(rx);
        });

        *server_runtime = Some(ServerRuntime { tx, handle });
        println!("[Core] Servidor iniciado.");
        Ok(())
    }

    pub fn stop_server(server_runtime: &mut Option<ServerRuntime>) {
        if let Some(runtime) = server_runtime.take() {
            let _ = runtime.tx.send(RunnerCommand::Stop);
            let _ = runtime.handle.join();
        }
    }
}