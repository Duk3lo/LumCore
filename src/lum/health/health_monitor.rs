use std::{
    env::consts::{ARCH, OS},
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

// --- CONSTANTES DE COLOR ANSI PARA UN LOOK "HACKER / PREMIUM" ---
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[38;5;196m";
const GREEN: &str = "\x1b[38;5;46m";
const YELLOW: &str = "\x1b[38;5;226m";
const BLUE: &str = "\x1b[38;5;39m";
const MAGENTA: &str = "\x1b[38;5;213m";
const CYAN: &str = "\x1b[38;5;51m";
const ORANGE: &str = "\x1b[38;5;208m";

pub struct HealthMonitor {
    server_pid: Option<u32>,
    server_start_time: Option<Instant>,
    tps_strikes: u32,
    last_check_time: Option<Instant>,
    current_interval: Duration,
    enabled: bool,
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self {
            server_pid: None,
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

    pub fn set_server_pid(&mut self, pid: u32) {
        self.server_pid = Some(pid);
    }

    pub fn start(&mut self, cfg: &HealingConfig) {
        if !cfg.enable {
            self.enabled = false;
            println!("{YELLOW}⚠️  [HEALTH] Monitor de salud desactivado por configuración.{RESET}");
            return;
        }

        let interval_ms = parse_time_ms(&cfg.check_interval).unwrap_or(60_000);
        self.current_interval = Duration::from_millis(interval_ms);
        self.last_check_time = Some(Instant::now());
        self.enabled = true;

        println!(
            "{GREEN}🚀 [HEALTH] Escudo Sentinel ACTIVADO. Escaneos de rendimiento cada {}{RESET}",
            format_time(interval_ms / 1000)
        );
    }

    pub fn stop(&mut self) {
        self.enabled = false;
        self.server_pid = None;
        self.server_start_time = None;
        self.tps_strikes = 0;
        self.last_check_time = None;
        println!("{RED}🛑 [HEALTH] Escudo Sentinel DETENIDO.{RESET}");
    }

    pub fn notify_server_started(&mut self) {
        self.server_start_time = Some(Instant::now());
        self.tps_strikes = 0;
        self.last_check_time = Some(Instant::now());
        println!("{CYAN}⏱️  [HEALTH] Reloj de actividad y telemetría sincronizados.{RESET}");
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
        if !config.enable { return; }
        let Some(start) = self.server_start_time else { return; };

        let uptime = start.elapsed();
        let initial_delay_ms = parse_time_ms(&config.initial_delay).unwrap_or(0);
        if uptime < Duration::from_millis(initial_delay_ms) { return; }

        let scheduled_restart_ms = parse_time_ms(&config.scheduled_restart).unwrap_or(0);
        if scheduled_restart_ms > 0 && uptime >= Duration::from_millis(scheduled_restart_ms) {
            println!(
                "{MAGENTA}🔄 [HEALTH] Vida útil máxima alcanzada ({}). Ejecutando purga programada...{RESET}",
                config.scheduled_restart
            );
            self.execute_restart(server_cfg, server_runtime, core_tx);
            return;
        }

        if let Some(runtime) = server_runtime.as_ref() {
            let _ = runtime.tx.send(RunnerCommand::Input("world perf".to_string()));
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
        if !self.enabled || !config.enable || line.is_empty() { return; }
        if !line.contains("TPS (1 min):") { return; }

        let Some(idx) = line.find("Avg:") else { return; };
        let tail = &line[idx + 4..];
        let avg_str = tail.split(',').next().unwrap_or("").trim();

        if let Ok(avg_tps) = avg_str.parse::<f64>() {
            if avg_tps < config.min_tps_threshold {
                self.tps_strikes += 1;
                let bar = create_strike_bar(self.tps_strikes, config.max_strikes);
                
                println!(
                    "{ORANGE}⚠️  [HEALTH] DEGRADACIÓN DETECTADA: TPS={:.2} | Strikes: {} {}/{} {RESET}",
                    avg_tps, bar, self.tps_strikes, config.max_strikes
                );

                if self.tps_strikes >= config.max_strikes {
                    println!("{RED}{BOLD}🚨 [HEALTH] FALLO CRÍTICO DE RENDIMIENTO. INICIANDO RESCATE DE EMERGENCIA...{RESET}");
                    self.execute_restart(server_cfg, server_runtime, core_tx);
                }
            } else if self.tps_strikes > 0 {
                println!("{GREEN}✅ [HEALTH] Estabilización confirmada ({:.2} TPS). Cancelando alertas de strike.{RESET}", avg_tps);
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
        self.server_pid = None;

        CoreApp::stop_server(server_runtime);
        println!("{CYAN}⚙️  [HEALTH] Reconstruyendo entorno. Levantando servidor...{RESET}");

        if let Err(e) = CoreApp::start_server(server_cfg, server_runtime, core_tx) {
            println!("{RED}❌ [Core Error] Reconstrucción fallida: {e}{RESET}");
        } else {
            self.notify_server_started();
        }
    }

    // =========================================================================
    // ✨ DASHBOARD VISUAL (MAGIA DE RUST Y ANSI COLORS)
    // =========================================================================
    pub fn print_health_status(&self, server_is_active: bool) {
        let app_name = std::env::args()
            .next()
            .and_then(|p| PathBuf::from(p).file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "CoreNexus".to_string());

        let pid = std::process::id();
        let app_mem = get_process_memory_usage(pid);
        let cpu_load = get_system_cpu_load().unwrap_or_else(|| "N/A".to_string());
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        
        let status_color = if self.enabled { GREEN } else { DIM };
        let status_icon = if self.enabled { "⚡ ACTIVE" } else { "💤 OFFLINE" };
        let server_icon = if server_is_active { format!("{GREEN}🟢 RUNNING{RESET}") } else { format!("{RED}🔴 DOWN{RESET}") };

        println!("\n{BLUE}╭──────────────────────────────────────────────────────────────╮{RESET}");
        println!("{BLUE}│ {BOLD}{MAGENTA}🌌 CORENEXUS TELEMETRY {RESET}{BLUE}│ {ORANGE}🦀 Powered by Rust (Blazing Fast) {BLUE}│{RESET}");
        println!("{BLUE}├──────────────────────────────────────────────────────────────┤{RESET}");
        
        // Host Info
        println!("{BLUE}│ {CYAN}🖥️  HOST SYSTEM {RESET}");
        println!("{BLUE}│ {DIM}OS Target : {RESET}{}{RESET} ({})", OS.to_uppercase(), ARCH.to_uppercase());
        println!("{BLUE}│ {DIM}CPU Load  : {RESET}{}{RESET}", cpu_load);
        println!("{BLUE}│ {DIM}Cores Avail: {RESET}{}{RESET}", threads);
        
        println!("{BLUE}├──────────────────────────────────────────────────────────────┤{RESET}");
        // App Info
        println!("{BLUE}│ {MAGENTA}🛡️  {} PROCESS {RESET}", app_name.to_uppercase());
        println!("{BLUE}│ {DIM}PID       : {RESET}{}{RESET}", pid);
        println!("{BLUE}│ {DIM}RAM Usage :{RESET} {GREEN}{}{RESET} {DIM}(Zero-Cost Abstractions){RESET}", format_memory(&app_mem));
        
        println!("{BLUE}├──────────────────────────────────────────────────────────────┤{RESET}");
        // Monitor Info
        println!("{BLUE}│ {YELLOW}🩺 SENTINEL HEALER {RESET}");
        println!("{BLUE}│ {DIM}Status    : {RESET}{}{}{RESET}", status_color, status_icon);
        
        if self.enabled {
            let strike_bar = create_strike_bar(self.tps_strikes, 3);
            println!("{BLUE}│ {DIM}TPS Strikes: {RESET}{}", strike_bar);
            
            if let Some(last) = self.last_check_time {
                let elapsed = last.elapsed();
                let remaining = if elapsed >= self.current_interval { Duration::ZERO } else { self.current_interval - elapsed };
                println!("{BLUE}│ {DIM}Next Scan : {RESET}En {}", format_time(remaining.as_secs()));
            }
        }

        println!("{BLUE}├──────────────────────────────────────────────────────────────┤{RESET}");
        // Server Info
        println!("{BLUE}│ {GREEN}🎮 SERVER {RESET}");
        println!("{BLUE}│ {DIM}State     : {RESET}{}", server_icon);
        
        if let Some(s_pid) = self.server_pid {
            let mem = get_process_memory_usage(s_pid);
            println!("{BLUE}│ {DIM}JVM PID   : {RESET}{}", s_pid);
            println!("{BLUE}│ {DIM}JVM RAM   : {RESET}{}", format_memory(&mem));
            println!("{BLUE}│ {DIM}JVM CPU   : {RESET}{}", get_process_cpu_usage(s_pid));
            println!("{BLUE}│ {DIM}Threads   : {RESET}{}", get_process_thread_count(s_pid));
            println!("{BLUE}│ {DIM}Heap Info : {RESET}{}", get_jvm_heap_info(s_pid));
        }

        println!("{BLUE}╰──────────────────────────────────────────────────────────────╯{RESET}\n");
    }

    pub fn server_stopped(&mut self) {
        self.server_pid = None;
        self.server_start_time = None;
        self.tps_strikes = 0;
        self.last_check_time = Some(Instant::now());
    }
}

// --- UTILIDADES ---

fn create_strike_bar(current: u32, max: u32) -> String {
    let max = if max == 0 { 1 } else { max };
    let mut bar = format!("{DIM}[{RESET}");
    for i in 0..max {
        if i < current { bar.push_str(&format!("{RED}█{RESET}")); } else { bar.push_str(&format!("{GREEN}░{RESET}")); }
    }
    bar.push_str(&format!("{DIM}]{RESET}"));
    bar
}

fn parse_time_ms(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() { return None; }
    let mut digits = String::new();
    let mut unit = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() { digits.push(ch); } else if !ch.is_whitespace() { unit.push(ch); }
    }
    let value: u64 = digits.parse().ok()?;
    match unit.to_uppercase().as_str() {
        "MS" => Some(value), "S" => Some(value * 1_000), "M" => Some(value * 60_000),
        "H" => Some(value * 3_600_000), "D" => Some(value * 86_400_000), _ => None,
    }
}

fn format_time(total_secs: u64) -> String {
    if total_secs == 0 { return "0s".to_string(); }
    let d = total_secs / 86_400; let h = (total_secs % 86_400) / 3_600;
    let m = (total_secs % 3_600) / 60; let s = total_secs % 60;
    let mut out = String::new();
    if d > 0 { out.push_str(&format!("{d}d ")); }
    if h > 0 { out.push_str(&format!("{h}h ")); }
    if m > 0 { out.push_str(&format!("{m}m ")); }
    out.push_str(&format!("{s}s"));
    out.trim().to_string()
}

fn format_memory(kb_str: &str) -> String {
    if kb_str == "N/A" || kb_str.trim().is_empty() { return "N/A".to_string(); }
    match kb_str.trim().parse::<u64>() {
        Ok(kb) if kb >= 1024 * 1024 => format!("{:.2} GB", kb as f64 / (1024.0 * 1024.0)),
        Ok(kb) if kb >= 1024 => format!("{:.2} MB", kb as f64 / 1024.0),
        Ok(kb) => format!("{kb} KB"),
        Err(_) => "N/A".to_string(),
    }
}

fn get_process_memory_usage(pid: u32) -> String {
    if cfg!(windows) {
        let filter = format!("PID eq {pid}");
        if let Ok(output) = Command::new("tasklist").args(["/FI", filter.as_str(), "/FO", "CSV", "/NH"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            let cleaned = text.lines().next().unwrap_or("").split(',').last().unwrap_or("")
                .replace('"', "").replace(" K", "").replace(" KB", "").trim().to_string();
            let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() { return digits; }
        }
    } else {
        if let Ok(output) = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() { return text; }
        }
    }
    "N/A".to_string()
}

fn get_process_cpu_usage(pid: u32) -> String {
    if cfg!(windows) {
        if let Ok(output) = Command::new("wmic").args(["path", "Win32_PerfFormattedData_PerfProc_Process", "where", &format!("IDProcess={pid}"), "get", "PercentProcessorTime"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.eq_ignore_ascii_case("PercentProcessorTime") { return format!("{line}%"); }
            }
        }
    } else {
        if let Ok(output) = Command::new("ps").args(["-p", &pid.to_string(), "-o", "%cpu="]).output() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() { return format!("{text}%"); }
        }
    }
    "N/A".to_string()
}

fn get_process_thread_count(pid: u32) -> String {
    if cfg!(windows) {
        if let Ok(output) = Command::new("wmic").args(["process", "where", &format!("ProcessId={pid}"), "get", "ThreadCount", "/value"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some(value) = line.trim().strip_prefix("ThreadCount=") { return value.trim().to_string(); }
            }
        }
    } else {
        if let Ok(output) = Command::new("ps").args(["-o", "nlwp=", "-p", &pid.to_string()]).output() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() { return text; }
        }
    }
    "N/A".to_string()
}

fn get_jvm_heap_info(pid: u32) -> String {
    if let Ok(output) = Command::new("jcmd").arg(pid.to_string()).arg("GC.heap_info").output() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() { return text; }
    }
    "N/A".to_string()
}

fn get_system_cpu_load() -> Option<String> {
    if cfg!(windows) { None } else {
        if let Ok(output) = Command::new("sh").arg("-c").arg("awk '{print $1}' /proc/loadavg").output() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() { return Some(text); }
        }
        None
    }
}