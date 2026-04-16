use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::lum::config::watcher_config::WatcherConfig;
use crate::lum::core_app::CoreEvent;

use super::watcher::{initial_sync, is_temp_or_ignored, sync_entry, SyncAction, SyncState};

struct WorkerHandle {
    stop_tx: mpsc::Sender<()>,
    stop_flag: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

pub struct RunningWatcherGroup {
    workers: Vec<WorkerHandle>,
}

pub struct WatcherManager {
    watchers: HashMap<String, RunningWatcherGroup>,
}

impl WatcherManager {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub fn start_all(
        &mut self,
        config: &crate::lum::config::watcher_config::WatchersConfig,
        event_tx: mpsc::Sender<CoreEvent>,
    ) -> Result<(), String> {
        for (name, watcher_cfg) in &config.watchers {
            if watcher_cfg.enabled {
                self.start_named(name.clone(), watcher_cfg.clone(), event_tx.clone())?;
            }
        }
        Ok(())
    }

    pub fn start_named(
        &mut self,
        name: String,
        config: WatcherConfig,
        event_tx: mpsc::Sender<CoreEvent>,
    ) -> Result<(), String> {
        self.stop_named(&name);

        let destination = config
            .destination_path
            .clone()
            .ok_or_else(|| format!("Watcher '{name}' no tiene destination_path"))?;

        if config.source_paths.is_empty() {
            return Err(format!("Watcher '{name}' no tiene source_paths"));
        }

        fs::create_dir_all(&destination)
            .map_err(|e| format!("No pude crear destino {:?}: {e}", destination))?;

        let shared_state = Arc::new(SyncState::new());
        let mut group = RunningWatcherGroup { workers: Vec::new() };

        for source_root in &config.source_paths {
            let source_root = source_root.clone();

            if !source_root.exists() {
                Self::stop_workers(&mut group.workers);
                return Err(format!("La ruta de origen no existe: {:?}", source_root));
            }

            if config.copy_on_init {
                initial_sync(&source_root, &destination, &config.extensions, &shared_state)?;
            }

            let worker = spawn_worker(
                source_root.clone(),
                destination.clone(),
                config.clone(),
                shared_state.clone(),
                event_tx.clone(),
                true,
            );
            group.workers.push(worker);

            if config.multi_sync {
                let reverse_worker = spawn_worker(
                    destination.clone(),
                    source_root,
                    config.clone(),
                    shared_state.clone(),
                    event_tx.clone(),
                    false,
                );
                group.workers.push(reverse_worker);
            }
        }

        self.watchers.insert(name, group);
        Ok(())
    }

    pub fn stop_named(&mut self, name: &str) {
        if let Some(mut group) = self.watchers.remove(name) {
            Self::stop_workers(&mut group.workers);
        }
    }

    pub fn stop_all(&mut self) {
        let keys: Vec<String> = self.watchers.keys().cloned().collect();
        for key in keys {
            self.stop_named(&key);
        }
    }

    fn stop_workers(workers: &mut Vec<WorkerHandle>) {
        for worker in workers.drain(..) {
            worker.stop_flag.store(true, Ordering::SeqCst);
            let _ = worker.stop_tx.send(());
            let _ = worker.join.join();
        }
    }
}

fn spawn_worker(
    source_root: PathBuf,
    destination_root: PathBuf,
    config: WatcherConfig,
    shared_state: Arc<SyncState>,
    event_tx: mpsc::Sender<CoreEvent>,
    allow_restart_signal: bool,
) -> WorkerHandle {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = Arc::clone(&stop_flag);

    let join = thread::spawn(move || {
        if let Err(err) = run_worker(
            source_root,
            destination_root,
            config,
            shared_state,
            stop_rx,
            stop_flag_thread,
            event_tx,
            allow_restart_signal,
        ) {
            eprintln!("[watcher] worker terminó con error: {err}");
        }
    });

    WorkerHandle {
        stop_tx,
        stop_flag,
        join,
    }
}

fn run_worker(
    source_root: PathBuf,
    destination_root: PathBuf,
    config: WatcherConfig,
    shared_state: Arc<SyncState>,
    stop_rx: mpsc::Receiver<()>,
    stop_flag: Arc<AtomicBool>,
    event_tx: mpsc::Sender<CoreEvent>,
    allow_restart_signal: bool,
) -> Result<(), String> {
    let (event_tx_backend, event_rx) = mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher = RecommendedWatcher::new(event_tx_backend, Config::default())
        .map_err(|e| format!("No se pudo crear watcher: {e}"))?;

    let mode = if config.watch_subfolders {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    watcher
        .watch(&source_root, mode)
        .map_err(|e| format!("No se pudo vigilar {:?}: {e}", source_root))?;

    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let debounce = Duration::from_millis(450);
    let tick = Duration::from_millis(100);

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        if stop_rx.try_recv().is_ok() {
            break;
        }

        match event_rx.recv_timeout(tick) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if is_temp_or_ignored(&path) {
                        continue;
                    }

                    if !path.starts_with(&source_root) {
                        continue;
                    }

                    pending.insert(path, Instant::now());
                }
            }
            Ok(Err(err)) => eprintln!("[watcher] error del backend: {err}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let due: Vec<PathBuf> = pending
            .iter()
            .filter_map(|(path, last_seen)| {
                if now.duration_since(*last_seen) >= debounce {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        for path in due {
            pending.remove(&path);

            match sync_entry(
                &source_root,
                &destination_root,
                &path,
                &config.extensions,
                &config.restart_server_extensions,
                &shared_state,
                &stop_flag,
            ) {
                Ok(SyncAction::None) => {}
                Ok(SyncAction::RestartServer) => {
                    if allow_restart_signal {
                        let _ = event_tx.send(CoreEvent::RestartRequested {
                            changed_path: path.clone(),
                        });
                    } else {
                        println!(
                            "[watcher] cambio sincronizado en modo espejo, no reinicio: {:?}",
                            path
                        );
                    }
                }
                Err(err) => eprintln!("[watcher] error sincronizando {:?}: {err}", path),
            }
        }
    }

    Ok(())
}