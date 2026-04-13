use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::lum::config::watcher_config::{WatcherConfig, WatchersConfig};
use super::watcher::{is_inside, is_temp_or_ignored, sync_path};

pub struct RunningWatcher {
    stop_tx: Sender<()>,
    join: Option<JoinHandle<()>>,
}

pub struct WatcherManager {
    watchers: HashMap<String, RunningWatcher>,
}

impl WatcherManager {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub fn start_all(&mut self, config: &WatchersConfig) -> Result<(), String> {
        for (name, watcher_cfg) in &config.watchers {
            if watcher_cfg.enabled {
                self.start_one(name.clone(), watcher_cfg.clone())?;
            }
        }
        Ok(())
    }

    pub fn start_one(&mut self, name: String, config: WatcherConfig) -> Result<(), String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join = thread::spawn(move || {
            if let Err(err) = run_watcher_thread(name, config, stop_rx) {
                eprintln!("Watcher detenido con error: {err}");
            }
        });

        self.watchers.insert(
            name,
            RunningWatcher {
                stop_tx,
                join: Some(join),
            },
        );

        Ok(())
    }

    pub fn stop_one(&mut self, name: &str) {
        if let Some(mut running) = self.watchers.remove(name) {
            let _ = running.stop_tx.send(());
            if let Some(join) = running.join.take() {
                let _ = join.join();
            }
        }
    }

    pub fn stop_all(&mut self) {
        let keys: Vec<String> = self.watchers.keys().cloned().collect();
        for key in keys {
            self.stop_one(&key);
        }
    }
}

fn run_watcher_thread(
    name: String,
    config: WatcherConfig,
    stop_rx: Receiver<()>,
) -> Result<(), String> {
    let destination = config
        .destination_path
        .clone()
        .ok_or_else(|| format!("Watcher '{name}' no tiene destination_path"))?;

    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(
        event_tx,
        Config::default(),
    )
    .map_err(|e| format!("No se pudo crear watcher '{name}': {e}"))?;

    for source in &config.source_paths {
        let mode = if config.watch_subfolders {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(source, mode)
            .map_err(|e| format!("No se pudo vigilar {:?}: {e}", source))?;
    }

    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let quiet_for = Duration::from_millis(500);
    let poll_tick = Duration::from_millis(100);

    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }

        match event_rx.recv_timeout(poll_tick) {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if is_temp_or_ignored(&path) {
                        continue;
                    }

                    if is_inside(&path, &destination) {
                        continue;
                    }

                    pending.insert(path, Instant::now());
                }
            }
            Ok(Err(err)) => {
                eprintln!("Error en watcher '{name}': {err}");
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let due: Vec<PathBuf> = pending
            .iter()
            .filter_map(|(path, last_seen)| {
                if now.duration_since(*last_seen) >= quiet_for {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        for path in due {
            pending.remove(&path);

            if let Err(err) = sync_path(
                &config.source_paths[0],
                &destination,
                &path,
            ) {
                eprintln!("Error sincronizando {:?}: {}", path, err);
            }
        }
    }

    Ok(())
}