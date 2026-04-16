use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

use crate::lum::core_app::CoreEvent;
use super::config::jar_config::ServerConfig;

#[derive(Debug, Clone)]
pub enum RunnerCommand {
    Input(String),
    Stop,
}

pub struct JavaJarRunner {
    jar_name: String,
    jar_dir: PathBuf,
    jvm_args: Vec<String>,
    jar_args: Vec<String>,
}

impl JavaJarRunner {
    pub fn from_config(config: &ServerConfig) -> Result<Self, String> {
        let jar_path = config.jar_path.trim();
        if jar_path.is_empty() {
            return Err("La ruta del JAR está vacía".to_string());
        }

        let full_path = PathBuf::from(jar_path);
        let jar_name = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "La ruta debe incluir el nombre de un archivo .jar".to_string())?
            .to_string();

        if !jar_name.to_lowercase().ends_with(".jar") {
            return Err("La ruta debe terminar en .jar".to_string());
        }

        let jar_dir = full_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        Ok(Self {
            jar_name,
            jar_dir,
            jvm_args: config.jvm_args.clone(),
            jar_args: config.jar_args.clone(),
        })
    }

    pub fn start_and_read(&self, rx: Receiver<RunnerCommand>, core_tx: Sender<CoreEvent>) {
        println!("Starting JAR: {}", self.jar_name);
        println!("Working directory: {:?}", self.jar_dir);

        let mut command = Command::new("java");
        command.current_dir(&self.jar_dir);

        for jvm_arg in &self.jvm_args {
            if !jvm_arg.is_empty() { command.arg(jvm_arg); }
        }

        command.arg("-jar").arg(&self.jar_name);

        for jar_arg in &self.jar_args {
            if !jar_arg.is_empty() { command.arg(jar_arg); }
        }

        command.stdout(Stdio::piped());
        command.stdin(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(process) => process,
            Err(e) => {
                println!("Error starting Java process: {}", e);
                return;
            }
        };

        let pid = child.id();
        let _ = core_tx.send(CoreEvent::ServerStarted { pid });

        let stdout_handle = child.stdout.take().map(|out| {
            let tx = core_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines() {
                    match line {
                        Ok(text) => {
                            println!("[JAR] {}", text);
                            let _ = tx.send(CoreEvent::ServerLog(text));
                        }
                        Err(e) => {
                            println!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            })
        });

        let stderr_handle = child.stderr.take().map(|err| {
            thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines() {
                    match line {
                        Ok(text) => eprintln!("[JAR ERR] {}", text),
                        Err(e) => {
                            eprintln!("Error reading stderr: {}", e);
                            break;
                        }
                    }
                }
            })
        });

        let mut jar_stdin = child.stdin.take();
        let mut stop_requested = false;

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(RunnerCommand::Input(cmd)) => {
                    if let Some(stdin) = jar_stdin.as_mut() {
                        let _ = stdin.write_all(cmd.as_bytes());
                        let _ = stdin.write_all(b"\n");
                        let _ = stdin.flush();
                    }
                }
                Ok(RunnerCommand::Stop) => {
                    stop_requested = true;
                    if let Some(stdin) = jar_stdin.as_mut() {
                        let _ = stdin.write_all(b"stop\n");
                        let _ = stdin.flush();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => { stop_requested = true; }
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    println!("JAR finished with status: {}", status);
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    println!("Error checking Java process: {}", e);
                    break;
                }
            }

            if stop_requested {
                for _ in 0..10 {
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) => thread::sleep(Duration::from_millis(50)),
                        Err(_) => break,
                    }
                }

                if let Ok(None) = child.try_wait() { let _ = child.kill(); }
                let _ = child.wait();
                break;
            }
        }

        if let Some(handle) = stdout_handle { let _ = handle.join(); }
        if let Some(handle) = stderr_handle { let _ = handle.join(); }
    }
}