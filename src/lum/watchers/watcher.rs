use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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

pub fn is_inside(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}

pub fn wait_until_stable(
    path: &Path,
    quiet_for: Duration,
    max_wait: Duration,
    poll_every: Duration,
) -> FileSnapshot {
    let start = Instant::now();
    let mut last_change = Instant::now();
    let mut last = snapshot(path);

    loop {
        if start.elapsed() >= max_wait {
            return last;
        }

        thread::sleep(poll_every);

        let current = snapshot(path);

        if current != last {
            last = current;
            last_change = Instant::now();
            continue;
        }

        if last_change.elapsed() >= quiet_for {
            return last;
        }
    }
}

pub fn sync_path(
    source_root: &Path,
    destination_root: &Path,
    changed_path: &Path,
) -> Result<(), String> {
    if !changed_path.starts_with(source_root) {
        return Ok(());
    }

    let relative = changed_path
        .strip_prefix(source_root)
        .map_err(|e| format!("No se pudo relativizar la ruta: {e}"))?;

    let target_path = destination_root.join(relative);

    let snap = wait_until_stable(
        changed_path,
        Duration::from_millis(800),
        Duration::from_secs(10),
        Duration::from_millis(150),
    );

    if snap.exists {
        if snap.is_dir {
            if target_path.is_file() {
                fs::remove_file(&target_path).ok();
            }
            fs::create_dir_all(&target_path)
                .map_err(|e| format!("No pude crear carpeta destino {:?}: {e}", target_path))?;
            return Ok(());
        }

        if target_path.is_dir() {
            fs::remove_dir_all(&target_path).ok();
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No pude crear carpeta padre {:?}: {e}", parent))?;
        }

        fs::copy(changed_path, &target_path)
            .map_err(|e| format!("No pude copiar {:?} -> {:?}: {e}", changed_path, target_path))?;

        return Ok(());
    }

    if target_path.is_dir() {
        fs::remove_dir_all(&target_path)
            .map_err(|e| format!("No pude borrar carpeta destino {:?}: {e}", target_path))?;
    } else if target_path.exists() {
        fs::remove_file(&target_path)
            .map_err(|e| format!("No pude borrar archivo destino {:?}: {e}", target_path))?;
    }

    Ok(())
}