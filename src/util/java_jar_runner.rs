use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Write};
use std::sync::mpsc::Receiver;
use std::thread;
use std::path::{Path, PathBuf};

pub struct JavaJarRunner {
    jar_name: String,
    jar_dir: PathBuf,
}

impl JavaJarRunner {
    pub fn load(path: &str) -> Self {
        let full_path = PathBuf::from(path);
        let jar_name = full_path
            .file_name()
            .expect("La ruta debe incluir el nombre de un archivo .jar")
            .to_string_lossy()
            .to_string();

        let jar_dir = full_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        JavaJarRunner {
            jar_name,
            jar_dir,
        }
    }

    pub fn start_and_read(&self, rx: Receiver<String>) {
        println!("Starting JAR: {}", self.jar_name);
        println!("Working directory: {:?}", self.jar_dir);

        let mut command = Command::new("java");
        
        command
            .current_dir(&self.jar_dir)
            .arg("-jar")
            .arg(&self.jar_name)
            .arg("--assets")
            .arg("../Assets.zip")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(process) => process,
            Err(e) => {
                println!("Error starting Java process: {}", e);
                return;
            }
        };

        if let Some(mut jar_stdin) = child.stdin.take() {
            thread::spawn(move || {
                for cmd in rx {
                    let formatted_cmd = format!("{}\n", cmd);
                    if jar_stdin.write_all(formatted_cmd.as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }

        if let Some(jar_stdout) = child.stdout.take() {
            let reader = BufReader::new(jar_stdout);

            for line in reader.lines() {
                match line {
                    Ok(text) => println!("[JAR LOG]: {}", text),
                    Err(e) => println!("Error reading output: {}", e),
                }
            }
        }

        let exit_status = child.wait().unwrap();
        println!("JAR finished with status: {}", exit_status);
    }
}