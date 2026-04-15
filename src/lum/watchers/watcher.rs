use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    exists: bool,
    is_dir: bool,
    len: u64,
    modified_ns: Option<u128>,
}

fn snapshot(path: &Path) -> FileSnapshot {
    match fs::metadata(path) {
        Ok(meta) => {
            let modified_ns = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos());

            FileSnapshot {
                exists: true,
                is_dir: meta.is_dir(),
                len: meta.len(),
                modified_ns,
            }
        }
        Err(_) => FileSnapshot {
            exists: false,
            is_dir: false,
            len: 0,
            modified_ns: None,
        },
    }
}

fn normalize_ext(s: &str) -> String {
    s.trim().trim_start_matches('.').to_lowercase()
}

pub fn is_temp_or_ignored(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    let lower = name.to_lowercase();

    lower.ends_with(".swp")
        || lower.ends_with(".swx")
        || lower.ends_with(".tmp")
        || lower.ends_with(".temp")
        || lower.ends_with(".part")
        || lower.ends_with(".crdownload")
        || lower.ends_with('~')
        || lower.starts_with(".~")
        || lower.starts_with(".git")
}

pub fn has_allowed_extension(path: &Path, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }

    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };

    let ext = normalize_ext(ext);
    extensions.iter().any(|allowed| normalize_ext(allowed) == ext)
}

pub fn should_restart_server(path: &Path, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return false;
    }

    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };

    let ext = normalize_ext(ext);
    extensions.iter().any(|allowed| normalize_ext(allowed) == ext)
}

fn wait_until_stable(
    path: &Path,
    quiet_for: Duration,
    max_wait: Duration,
    poll_every: Duration,
    stop_flag: &AtomicBool,
) -> Option<FileSnapshot> {
    let start = Instant::now();
    let mut last_change = Instant::now();
    let mut last = snapshot(path);

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return None;
        }

        if start.elapsed() >= max_wait {
            return Some(last);
        }

        thread::sleep(poll_every);

        if stop_flag.load(Ordering::Relaxed) {
            return None;
        }

        let current = snapshot(path);
        if current != last {
            last = current;
            last_change = Instant::now();
            continue;
        }

        if last_change.elapsed() >= quiet_for {
            return Some(last);
        }
    }
}

pub struct SyncState {
    ignored: Mutex<Vec<(PathBuf, Instant)>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            ignored: Mutex::new(Vec::new()),
        }
    }

    pub fn ignore_for(&self, path: impl Into<PathBuf>, ttl: Duration) {
        let until = Instant::now() + ttl;
        if let Ok(mut list) = self.ignored.lock() {
            list.push((path.into(), until));
        }
    }

    pub fn should_ignore(&self, path: &Path) -> bool {
        let now = Instant::now();
        let Ok(mut list) = self.ignored.lock() else {
            return false;
        };

        list.retain(|(_, until)| *until > now);
        list.iter().any(|(ignored, _)| path.starts_with(ignored))
    }
}

pub enum SyncAction {
    None,
    RestartServer,
}

fn delete_target(target_path: &Path) -> Result<(), String> {
    if target_path.is_dir() {
        fs::remove_dir_all(target_path)
            .map_err(|e| format!("No pude borrar carpeta destino {:?}: {e}", target_path))?;
    } else if target_path.exists() {
        fs::remove_file(target_path)
            .map_err(|e| format!("No pude borrar archivo destino {:?}: {e}", target_path))?;
    }

    Ok(())
}

pub fn sync_entry(
    source_root: &Path,
    destination_root: &Path,
    changed_path: &Path,
    extensions: &[String],
    restart_server_extensions: &[String],
    state: &SyncState,
    stop_flag: &AtomicBool,
) -> Result<SyncAction, String> {
    if state.should_ignore(changed_path) {
        return Ok(SyncAction::None);
    }

    if !changed_path.starts_with(source_root) {
        return Ok(SyncAction::None);
    }

    let restart_after_sync = should_restart_server(changed_path, restart_server_extensions);

    let snap = match wait_until_stable(
        changed_path,
        Duration::from_millis(700),
        Duration::from_secs(8),
        Duration::from_millis(120),
        stop_flag,
    ) {
        Some(s) => s,
        None => return Ok(SyncAction::None),
    };

    let relative = changed_path
        .strip_prefix(source_root)
        .map_err(|e| format!("No se pudo relativizar la ruta: {e}"))?;

    let target_path = destination_root.join(relative);
    let ignore_ttl = Duration::from_secs(2);

    if !snap.exists {
        state.ignore_for(&target_path, ignore_ttl);
        delete_target(&target_path)?;
        return Ok(if restart_after_sync {
            SyncAction::RestartServer
        } else {
            SyncAction::None
        });
    }

    if snap.is_dir {
        state.ignore_for(&target_path, ignore_ttl);

        if target_path.is_file() {
            fs::remove_file(&target_path).ok();
        }

        fs::create_dir_all(&target_path)
            .map_err(|e| format!("No pude crear carpeta destino {:?}: {e}", target_path))?;

        return Ok(SyncAction::None);
    }

    if !has_allowed_extension(changed_path, extensions) {
        return Ok(SyncAction::None);
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("No pude crear carpeta padre {:?}: {e}", parent))?;
    }

    state.ignore_for(&target_path, ignore_ttl);

    if target_path.is_dir() {
        fs::remove_dir_all(&target_path).ok();
    }

    fs::copy(changed_path, &target_path)
        .map_err(|e| format!("No pude copiar {:?} -> {:?}: {e}", changed_path, target_path))?;

    if restart_after_sync {
        return Ok(SyncAction::RestartServer);
    }

    Ok(SyncAction::None)
}

pub fn initial_sync(
    source_root: &Path,
    destination_root: &Path,
    extensions: &[String],
    state: &SyncState,
) -> Result<(), String> {
    if !source_root.exists() {
        return Ok(());
    }

    fn walk(
        root: &Path,
        current: &Path,
        destination_root: &Path,
        extensions: &[String],
        state: &SyncState,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current)
            .map_err(|e| format!("No pude leer directorio {:?}: {e}", current))?
        {
            let entry = entry.map_err(|e| format!("Error leyendo entrada: {e}"))?;
            let path = entry.path();
            let meta = entry
                .metadata()
                .map_err(|e| format!("No pude leer metadata {:?}: {e}", path))?;

            let relative = path
                .strip_prefix(root)
                .map_err(|e| format!("No pude relativizar {:?}: {e}", path))?;

            let target = destination_root.join(relative);

            if meta.is_dir() {
                state.ignore_for(&target, Duration::from_secs(2));
                fs::create_dir_all(&target)
                    .map_err(|e| format!("No pude crear carpeta destino {:?}: {e}", target))?;
                walk(root, &path, destination_root, extensions, state)?;
            } else if has_allowed_extension(&path, extensions) {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("No pude crear carpeta padre {:?}: {e}", parent))?;
                }

                state.ignore_for(&target, Duration::from_secs(2));
                fs::copy(&path, &target)
                    .map_err(|e| format!("No pude copiar {:?} -> {:?}: {e}", path, target))?;
            }
        }

        Ok(())
    }

    walk(source_root, source_root, destination_root, extensions, state)
}