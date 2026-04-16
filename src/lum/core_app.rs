use crate::lum::api::updater::UpdaterManager;
use crate::lum::commands::{self, CoreContext};
use crate::lum::config::curseforge_config::CurseForgeConfig;
use crate::lum::config::github_config::GitHubConfig;
use crate::lum::config::healing_config::HealingConfig;
use crate::lum::config::jar_config::{ConfigLocation, ServerConfig};
use crate::lum::config::updates_config::UpdatesConfig;
use crate::lum::config::watcher_config::WatchersConfig;
use crate::lum::health::health_monitor::HealthMonitor;
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
    ServerStarted { pid: u32 },
    ServerLog(String),
}

impl CoreApp {
    pub fn start() {
        println!("--- Starting CoreNexus (Rust Edition) ---");

        let mut server_cfg =
            ServerConfig::load_or_create(ConfigLocation::Local).unwrap_or_default();
        let mut watchers_cfg = WatchersConfig::load_or_create().unwrap();
        let mut curseforge_cfg = CurseForgeConfig::load_or_create().unwrap();
        let mut github_cfg = GitHubConfig::load_or_create().unwrap();
        let mut updates_cfg = UpdatesConfig::load_or_create().unwrap();

        let mut healing_cfg = HealingConfig::load_or_create().unwrap_or_default();
        let mut health_monitor = HealthMonitor::new();
        health_monitor.start(&healing_cfg);

        let mut watcher_manager = WatcherManager::new();
        let mut updater_manager = UpdaterManager::new();
        updater_manager.start(updates_cfg.clone());

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

        let _ = watcher_manager.start_all(&watchers_cfg, core_tx.clone());

        let mut server_runtime: Option<ServerRuntime> = None;
        let mut last_restart = Instant::now() - Duration::from_secs(10);

        println!("[Core] Ready. Type commands (type 'exit' to quit).");
        commands::print_help();

        loop {
            let event_opt = match core_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ev) => Some(ev),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            };

            health_monitor.tick(
                &healing_cfg,
                &mut server_runtime,
                &server_cfg,
                core_tx.clone(),
            );

            if let Some(event) = event_opt {
                match event {
                    CoreEvent::UserCommand(input) => {
                        let cmd = input.trim();
                        if cmd.is_empty() {
                            continue;
                        }

                        if cmd == "exit" || cmd == "stop" {
                            println!("[Core] Shutting down...");
                            let _ = curseforge_cfg.save();
                            let _ = github_cfg.save();
                            let _ = updates_cfg.save();
                            let _ = healing_cfg.save();
                            let _ = server_cfg.save();

                            health_monitor.stop();
                            updater_manager.stop();
                            Self::stop_server(&mut server_runtime);
                            watcher_manager.stop_all();
                            break;
                        }

                        let mut ctx = CoreContext {
                            server_cfg: &mut server_cfg,
                            watchers_cfg: &mut watchers_cfg,
                            curseforge_cfg: &mut curseforge_cfg,
                            github_cfg: &mut github_cfg,
                            updates_cfg: &mut updates_cfg,
                            healing_cfg: &mut healing_cfg,
                            health_monitor: &mut health_monitor,
                            updater_manager: &mut updater_manager,
                            watcher_manager: &mut watcher_manager,
                            server_runtime: &mut server_runtime,
                            event_tx: &core_tx,
                        };

                        if commands::dispatch(cmd, &mut ctx) {
                            if cmd.starts_with("core healing") {
                                let _ = ctx.healing_cfg.save();
                            }
                            continue;
                        }

                        if let Some(runtime) = ctx.server_runtime.as_ref() {
                            let _ = runtime.tx.send(RunnerCommand::Input(cmd.to_string()));
                        } else {
                            println!("[Core] No hay servidor activo. Usa 'jar start'.");
                        }
                    }

                    CoreEvent::RestartRequested { changed_path } => {
                        if last_restart.elapsed() < Duration::from_millis(1200) {
                            println!("[Watcher] Reinicio omitido por rebote: {:?}", changed_path);
                            continue;
                        }

                        last_restart = Instant::now();
                        println!("[Watcher] Reiniciando por cambio en: {:?}", changed_path);

                        let was_running = server_runtime.is_some();
                        if was_running {
                            Self::stop_server(&mut server_runtime);
                        }

                        if server_cfg.auto_restart || was_running {
                            if let Err(e) = Self::start_server(
                                &server_cfg,
                                &mut server_runtime,
                                core_tx.clone(),
                            ) {
                                println!("[Core Error] {e}");
                            }
                        }
                    }

                    CoreEvent::ServerStarted { pid } => {
                        health_monitor.set_server_pid(pid);
                        health_monitor.notify_server_started();
                        println!("[Core] PID del servidor detectado: {pid}");
                    }

                    CoreEvent::ServerLog(line) => {
                        health_monitor.process_server_log(
                            &line,
                            &healing_cfg,
                            &mut server_runtime,
                            &server_cfg,
                            core_tx.clone(),
                        );
                    }
                }
            }
        }

        println!("--- Everything is safely shut down. Goodbye! ---");
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
        core_tx: mpsc::Sender<CoreEvent>,
    ) -> Result<(), String> {
        if server_runtime.is_some() {
            return Err("Ya está en ejecución".to_string());
        }

        let runner = JavaJarRunner::from_config(server_cfg)?;
        let (tx, rx) = mpsc::channel::<RunnerCommand>();

        let handle = thread::spawn(move || {
            runner.start_and_read(rx, core_tx);
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