use super::config::{ConfigLocation, ServerConfig};
use super::java_jar_runner::JavaJarRunner;
use std::sync::mpsc;
use std::thread;

pub struct CoreApp;

impl CoreApp {
    pub fn start() {
        println!("--- Starting CoreNexus (Rust Edition) ---");

        println!("[Core] Loading configurations...");
        let config = match ServerConfig::load_or_create(ConfigLocation::Local) {
            Ok(cfg) => cfg,
            Err(e) => {
                println!("[Core Error] Failed to load configuration: {}", e);
                return;
            }
        };

        let runner = JavaJarRunner::from_config(&config);

        let (tx, rx) = mpsc::channel::<String>();

        let handle = thread::spawn(move || {
            println!("[Core] Launching background thread for JAR...");
            runner.start_and_read(rx);
        });

        println!("[Core] Ready. Type commands to send to the server (type 'exit' to quit):");
        let stdin = std::io::stdin();

        for line in stdin.lines() {
            let input = line.unwrap();
            let cmd = input.trim();

            if cmd == "exit" || cmd == "stop" {
                println!("[Core] Shutting down... Sending 'stop' to JAR.");
                tx.send("stop".to_string()).unwrap();
                break;
            }

            tx.send(cmd.to_string()).unwrap();
        }
        
        println!("[Core] Waiting for the JAR to shut down completely...");
        handle.join().unwrap();
        println!("--- Everything is safely shut down. Goodbye! ---");
    }
}