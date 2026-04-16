use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use crate::lum::config::updates_config::UpdatesConfig;

pub struct UpdaterManager {
    running: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl UpdaterManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
        }
    }

    fn parse_time(time_str: &str) -> u64 {
        let text = time_str.trim().to_uppercase();
        if text.ends_with('H') {
            text.replace("H", "").parse::<u64>().unwrap_or(12) * 3600
        } else if text.ends_with('M') {
            text.replace("M", "").parse::<u64>().unwrap_or(30) * 60
        } else if text.ends_with('D') {
            text.replace("D", "").parse::<u64>().unwrap_or(1) * 86400
        } else {
            1800
        }
    }

    pub fn start(&mut self, config: UpdatesConfig) {
        if self.running.load(Ordering::SeqCst) { return; }
        self.running.store(true, Ordering::SeqCst);

        if config.curseforge.enable {
            let interval = Self::parse_time(&config.curseforge.check_interval);
            let running_cf = self.running.clone();
            
            self.threads.push(thread::spawn(move || {
                println!("[Updater] Auto-Check CurseForge iniciado (Cada {}s).", interval);
                while running_cf.load(Ordering::SeqCst) {
                    for _ in 0..interval {
                        if !running_cf.load(Ordering::SeqCst) { break; }
                        thread::sleep(Duration::from_secs(1));
                    }
                    if !running_cf.load(Ordering::SeqCst) { break; }
                    
                    println!("[Updater] Ejecutando sincronización automática de CurseForge...");
                }
            }));
        }

        if config.github.enable {
            let interval = Self::parse_time(&config.github.check_interval);
            let running_gh = self.running.clone();
            
            self.threads.push(thread::spawn(move || {
                println!("[Updater] Auto-Check GitHub iniciado (Cada {}s).", interval);
                while running_gh.load(Ordering::SeqCst) {
                    for _ in 0..interval {
                        if !running_gh.load(Ordering::SeqCst) { break; }
                        thread::sleep(Duration::from_secs(1));
                    }
                    if !running_gh.load(Ordering::SeqCst) { break; }
                    
                    println!("[Updater] Ejecutando sincronización automática de GitHub...");
                }
            }));
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        while let Some(handle) = self.threads.pop() {
            let _ = handle.join();
        }
        println!("[Updater] Todos los procesos en segundo plano detenidos.");
    }
}