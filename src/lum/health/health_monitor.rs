use std::{
    path::PathBuf,
    process::Command,
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::lum::{
    config::healing_config::HealingConfig,
    config::jar_config::ServerConfig,
    core_app::{CoreApp, CoreEvent, ServerRuntime},
    java_jar_runner::RunnerCommand,
};

pub struct HealthMonitor {
    server_start_time: Option<Instant>,
    tps_strikes: u32,
    last_check_time: Option<Instant>,
    current_interval: Duration,
    enabled: bool,
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self {
            server_start_time: None,
            tps_strikes: 0,
            last_check_time: None,
            current_interval: Duration::from_secs(60),
            enabled: false,
        }
    }
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, cfg: &HealingConfig) {
        if !cfg.enable {
            self.enabled = false;
            println!("[HEALTH] Monitor de salud desactivado.");
            return;
        }

        let interval_ms = parse_time_ms(&cfg.check_interval).unwrap_or(60_000);
        self.current_interval = Duration::from_millis(interval_ms);
        self.last_check_time = Some(Instant::now());
        self.enabled = true;

        println!(
            "[HEALTH] Monitor iniciado. Chequeos cada {}.",
            format_time(interval_ms / 1000)
        );
    }

    pub fn stop(&mut self) {
        self.enabled = false;
        self.server_start_time = None;
        self.tps_strikes = 0;
        self.last_check_time = None;
        println!("[HEALTH] Monitor de salud detenido.");
    }

    pub fn notify_server_started(&mut self) {
        self.server_start_time = Some(Instant::now());
        self.tps_strikes = 0;
        self.last_check_time = Some(Instant::now());
        println!("[HEALTH] Reloj de actividad del servidor reiniciado.");
    }

    pub fn tick(
        &mut self,
        config: &HealingConfig,
        server_runtime: &mut Option<ServerRuntime>,
        server_cfg: &ServerConfig,
        core_tx: mpsc::Sender<CoreEvent>,
    ) {
        if !self.enabled || !config.enable {
            return;
        }

        if server_runtime.is_some() && self.server_start_time.is_none() {
            self.notify_server_started();
        } else if server_runtime.is_none() && self.server_start_time.is_some() {
            self.server_start_time = None;
        }

        let last = *self.last_check_time.get_or_insert_with(Instant::now);
        if last.elapsed() < self.current_interval {
            return;
        }

        self.last_check_time = Some(Instant::now());
        self.perform_check(config, server_runtime, server_cfg, core_tx);
    }

    fn perform_check(
        &mut self,
        config: &HealingConfig,
        server_runtime: &mut Option<ServerRuntime>,
        server_cfg: &ServerConfig,
        core_tx: mpsc::Sender<CoreEvent>,
    ) {
        if !config.enable {
            return;
        }

        let Some(start) = self.server_start_time else {
            return;
        };

        let uptime = start.elapsed();
        let initial_delay_ms = parse_time_ms(&config.initial_delay).unwrap_or(0);
        if uptime < Duration::from_millis(initial_delay_ms) {
            return;
        }

        let scheduled_restart_ms = parse_time_ms(&config.scheduled_restart).unwrap_or(0);
        if scheduled_restart_ms > 0 && uptime >= Duration::from_millis(scheduled_restart_ms) {
            println!(
                "[HEALTH] Tiempo máximo alcanzado ({}). Iniciando reinicio programado...",
                config.scheduled_restart
            );
            self.execute_restart(server_cfg, server_runtime, core_tx);
            return;
        }

        if let Some(runtime) = server_runtime.as_ref() {
            let _ = runtime
                .tx
                .send(RunnerCommand::Input("world perf".to_string()));
        }
    }

    pub fn process_server_log(
        &mut self,
        line: &str,
        config: &HealingConfig,
        server_runtime: &mut Option<ServerRuntime>,
        server_cfg: &ServerConfig,
        core_tx: mpsc::Sender<CoreEvent>,
    ) {
        if !self.enabled || !config.enable || line.is_empty() {
            return;
        }

        if !line.contains("TPS (1 min):") {
            return;
        }

        let Some(idx) = line.find("Avg:") else {
            return;
        };

        let tail = &line[idx + 4..];
        let avg_str = tail.split(',').next().unwrap_or("").trim();

        if let Ok(avg_tps) = avg_str.parse::<f64>() {
            if avg_tps < config.min_tps_threshold {
                self.tps_strikes += 1;
                println!(
                    "[HEALTH] ¡ALERTA! TPS promedio bajó a {:.2}. Advertencia {}/{}",
                    avg_tps, self.tps_strikes, config.max_strikes
                );

                if self.tps_strikes >= config.max_strikes {
                    println!(
                        "[HEALTH] Límite de fallos de rendimiento alcanzado. Reinicio de emergencia..."
                    );
                    self.execute_restart(server_cfg, server_runtime, core_tx);
                }
            } else if self.tps_strikes > 0 {
                println!(
                    "[HEALTH] Rendimiento estabilizado ({:.2} TPS). Alertas canceladas.",
                    avg_tps
                );
                self.tps_strikes = 0;
            }
        }
    }

    fn execute_restart(
        &mut self,
        server_cfg: &ServerConfig,
        server_runtime: &mut Option<ServerRuntime>,
        core_tx: mpsc::Sender<CoreEvent>,
    ) {
        self.server_start_time = None;
        self.tps_strikes = 0;

        CoreApp::stop_server(server_runtime);
        println!("[HEALTH] Levantando servidor tras el reinicio...");

        if let Err(e) = CoreApp::start_server(server_cfg, server_runtime, core_tx) {
            println!("[Core Error] No se pudo reiniciar el servidor: {e}");
        } else {
            self.notify_server_started();
        }
    }

    pub fn print_health_status(&self, server_is_active: bool) {
        println!("=== Estado de Salud del Sistema ===");

        let app_name = std::env::args()
            .next()
            .and_then(|p| {
                PathBuf::from(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "CoreNexus".to_string());

        let pid = std::process::id();
        let app_mem = get_process_memory_usage(pid);
        let app_formatted_mem = format_memory(&app_mem);
        let cpu_load = get_system_cpu_load().unwrap_or_else(|| "No disponible".to_string());
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        println!("[APP] Proceso      : {} (PID: {})", app_name, pid);
        println!("[APP] RAM Core     : {}", app_formatted_mem);
        println!("[APP] Carga CPU    : {}", cpu_load);
        println!("[APP] Hilos Activos: {}", thread_count);

        println!(
            "[MONITOR] Estado    : {}",
            if self.enabled { "✅ ACTIVO" } else { "❌ APAGADO" }
        );

        if self.enabled {
            println!("[MONITOR] Fallos TPS : {}", self.tps_strikes);

            if let Some(last) = self.last_check_time {
                let elapsed = last.elapsed();
                let remaining = if elapsed >= self.current_interval {
                    Duration::from_secs(0)
                } else {
                    self.current_interval - elapsed
                };
                println!("[MONITOR] Prox. Check : en {}", format_time(remaining.as_secs()));
            }
        }

        println!(
            "[SERVER] Estado    : {}",
            if server_is_active { "✅ ACTIVO" } else { "❌ APAGADO" }
        );
        println!("===================================");
    }
}

fn parse_time_ms(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    let mut digits = String::new();
    let mut unit = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !ch.is_whitespace() {
            unit.push(ch);
        }
    }

    let value: u64 = digits.parse().ok()?;
    match unit.to_uppercase().as_str() {
        "MS" => Some(value),
        "S" => Some(value * 1_000),
        "M" => Some(value * 60_000),
        "H" => Some(value * 3_600_000),
        "D" => Some(value * 86_400_000),
        _ => None,
    }
}

fn format_time(total_secs: u64) -> String {
    if total_secs == 0 {
        return "0s".to_string();
    }

    let d = total_secs / 86_400;
    let h = (total_secs % 86_400) / 3_600;
    let m = (total_secs % 3_600) / 60;
    let s = total_secs % 60;

    let mut out = String::new();
    if d > 0 {
        out.push_str(&format!("{d}d "));
    }
    if h > 0 {
        out.push_str(&format!("{h}h "));
    }
    if m > 0 {
        out.push_str(&format!("{m}m "));
    }
    out.push_str(&format!("{s}s"));

    out.trim().to_string()
}

fn format_memory(kb_str: &str) -> String {
    if kb_str == "N/A" || kb_str.trim().is_empty() {
        return "N/A".to_string();
    }

    match kb_str.trim().parse::<u64>() {
        Ok(kb) if kb >= 1024 * 1024 => format!("{:.2} GB", kb as f64 / (1024.0 * 1024.0)),
        Ok(kb) if kb >= 1024 => format!("{:.2} MB", kb as f64 / 1024.0),
        Ok(kb) => format!("{kb} KB"),
        Err(_) => "N/A".to_string(),
    }
}

fn get_process_memory_usage(pid: u32) -> String {
    let is_win = cfg!(windows);

    let output = if is_win {
        let filter = format!("PID eq {pid}");
        Command::new("tasklist")
            .args(["/FI", filter.as_str(), "/FO", "CSV", "/NH"])
            .output()
    } else {
        Command::new("ps")
            .args(["-o", "rss=", "-p"])
            .arg(pid.to_string())
            .output()
    };

    let Ok(output) = output else {
        return "N/A".to_string();
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("").trim();

    if line.is_empty() {
        return "N/A".to_string();
    }

    if is_win {
        let cleaned = line
            .split(',')
            .last()
            .unwrap_or("")
            .replace('"', "")
            .replace(" K", "")
            .replace(" KB", "")
            .trim()
            .to_string();

        let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            "N/A".to_string()
        } else {
            digits
        }
    } else {
        line.to_string()
    }
}

fn get_system_cpu_load() -> Option<String> {
    if cfg!(windows) {
        None
    } else {
        let output = Command::new("sh")
            .arg("-c")
            .arg("awk '{print $1}' /proc/loadavg")
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}