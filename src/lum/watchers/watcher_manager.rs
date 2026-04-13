use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::lum::config::watcher_config::WatcherConfig;

use super::watcher::{initial_sync, is_temp_or_ignored, sync_entry, SyncAction, SyncState};

pub struct RunningWatcherGroup {
    stops: Vec<mpsc::Sender<()>>,
    joins: Vec<JoinHandle<()>>,
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

    pub fn start_all(&mut self, config: &crate::lum::config::watcher_config::WatchersConfig) -> Result<(), String> {
        for (name, watcher_cfg) in &config.watchers {
            if watcher_cfg.enabled {
                self.start_named(name.clone(), watcher_cfg.clone())?;
            }
        }
        Ok(())
    }

    pub fn start_named(&mut self, name: String, config: WatcherConfig) -> Result<(), String> {
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
        let mut group = RunningWatcherGroup {
            stops: Vec::new(),
            joins: Vec::new(),
        };

        for source_root in &config.source_paths {
            let source_root = source_root.clone();

            if !source_root.exists() {
                return Err(format!("La ruta de origen no existe: {:?}", source_root));
            }

            if config.copy_on_init {
                initial_sync(&source_root, &destination, &config.extensions, &shared_state)?;
            }

            let (tx, join) = spawn_worker(
                source_root.clone(),
                destination.clone(),
                config.clone(),
                shared_state.clone(),
            );

            group.stops.push(tx);
            group.joins.push(join);

            if config.multi_sync {
                let (tx_rev, join_rev) = spawn_worker(
                    destination.clone(),
                    source_root,
                    config.clone(),
                    shared_state.clone(),
                );

                group.stops.push(tx_rev);
                group.joins.push(join_rev);
            }
        }

        self.watchers.insert(name, group);
        Ok(())
    }

    pub fn stop_named(&mut self, name: &str) {
        if let Some(mut group) = self.watchers.remove(name) {
            for tx in group.stops.drain(..) {
                let _ = tx.send(());
            }

            for join in group.joins.drain(..) {
                let _ = join.join();
            }
        }
    }

    pub fn stop_all(&mut self) {
        let keys: Vec<String> = self.watchers.keys().cloned().collect();
        for key in keys {
            self.stop_named(&key);
        }
    }
}

fn spawn_worker(
    source_root: PathBuf,
    destination_root: PathBuf,
    config: WatcherConfig,
    shared_state: Arc<SyncState>,
) -> (mpsc::Sender<()>, JoinHandle<()>) {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let join = thread::spawn(move || {
        if let Err(err) = run_worker(source_root, destination_root, config, shared_state, stop_rx) {
            eprintln!("[watcher] worker terminó con error: {err}");
        }
    });

    (stop_tx, join)
}

fn run_worker(
    source_root: PathBuf,
    destination_root: PathBuf,
    config: WatcherConfig,
    shared_state: Arc<SyncState>,
    stop_rx: mpsc::Receiver<()>,
) -> Result<(), String> {
    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(event_tx, Config::default())
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
            ) {
                Ok(SyncAction::None) => {}
                Ok(SyncAction::RestartServer) => {
                    println!("Se detectó cambio que debería reiniciar el servidor: {:?}", path);
                }
                Err(err) => eprintln!("[watcher] error sincronizando {:?}: {err}", path),
            }
        }
    }

    Ok(())
}