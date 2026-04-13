
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::Receiver,
    thread,
};

use super::config::server_config::ServerConfig;

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

    pub fn start_and_read(&self, rx: Receiver<String>) {
        println!("Starting JAR: {}", self.jar_name);
        println!("Working directory: {:?}", self.jar_dir);

        let mut command = Command::new("java");
        command.current_dir(&self.jar_dir);

        for jvm_arg in &self.jvm_args {
            if !jvm_arg.is_empty() {
                command.arg(jvm_arg);
            }
        }

        command.arg("-jar").arg(&self.jar_name);

        for jar_arg in &self.jar_args {
            if !jar_arg.is_empty() {
                command.arg(jar_arg);
            }
        }

        command.stdout(Stdio::piped()).stdin(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(process) => process,
            Err(e) => {
                println!("Error starting Java process: {}", e);
                return;
            }
        };

        self.pipe_stdin(rx, &mut child);
        self.pipe_output(&mut child);
    }

    fn pipe_stdin(&self, rx: Receiver<String>, child: &mut Child) {
        let Some(mut jar_stdin) = child.stdin.take() else {
            println!("No se pudo abrir stdin del proceso Java");
            return;
        };

        thread::spawn(move || {
            for cmd in rx {
                let formatted_cmd = format!("{cmd}\n");
                if jar_stdin.write_all(formatted_cmd.as_bytes()).is_err() {
                    break;
                }
                let _ = jar_stdin.flush();
            }
        });
    }

    fn pipe_output(&self, child: &mut Child) {
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_handle = stdout.map(|out| {
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines() {
                    match line {
                        Ok(text) => println!("[JAR OUT] {}", text),
                        Err(e) => {
                            println!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            })
        });

        let stderr_handle = stderr.map(|err| {
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

        let exit_status = match child.wait() {
            Ok(status) => status,
            Err(e) => {
                println!("Error waiting Java process: {}", e);
                return;
            }
        };

        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        println!("JAR finished with status: {}", exit_status);
    }
}