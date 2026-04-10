mod util;
use util::java_jar_runner::JavaJarRunner;

use std::thread;
use std::sync::mpsc;

fn main() {
    println!("Main thread: Starting application...");
    
    let runner: JavaJarRunner = JavaJarRunner::load("/home/dukelo/Escritorio/Server/beat/Server/HytaleServer.jar");
    
    let (tx, rx) = mpsc::channel::<String>();
    
    let handle = thread::spawn(move || {
        println!("Background thread: Launching the JAR...");
        runner.start_and_read(rx);
    });
    println!("Main thread: Type commands to send to the server (type 'exit' to quit):");
    
    let stdin = std::io::stdin();
    
    for line in stdin.lines() {
        let input = line.unwrap();
        let cmd = input.trim();
        
        if cmd == "exit" {
            println!("Main thread: Shutting down... Sending 'stop' to JAR.");
            tx.send("stop".to_string()).unwrap();
            break;
        }

        tx.send(cmd.to_string()).unwrap();
        println!("Main thread: Sent command '{}' to JAR.", cmd);
    }

    println!("Main thread: Waiting for the JAR to shut down completely...");
    handle.join().unwrap();
    
    println!("Main thread: Everything is safely shut down. Goodbye!");
}