/* Author: Prudhvi Raj Belide
   Sept 2025
   File : Simulated device client sending telemetry and receiving commands */

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Bring shared message types
#[path = "../message.rs"]
mod message;
use message::{Message, MsgType};

fn main() {
    // Read command-line args
    let args: Vec<String> = std::env::args().collect();

    // Use provided device id or default
    let device_id = args
        .get(1)
        .map(|s| s.clone())
        .unwrap_or("device_1".to_string());

    // Enable silent fault mode if asked
    let simulate_fault = args.iter().any(|a| a == "--fault");

    println!("[{}] Connecting to broker...", device_id);

    // Connect to broker socket
    let stream =
        UnixStream::connect("/tmp/broker.sock").expect("Cannot connect to broker. Is it running?");

    // Separate write handle shared by sender logic
    let write_stream = Arc::new(Mutex::new(
        stream.try_clone().expect("Failed to clone stream"),
    ));

    println!("[{}] Connected. Sending telemetry...", device_id);

    // Background thread for incoming commands
    {
        let id = device_id.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stream);

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };

                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    if msg.msg_type == MsgType::Command {
                        println!("[{}] *** COMMAND RECEIVED: {} ***", id, msg.payload);

                        // Real device would parse and execute here
                    }
                }
            }
        });
    }

    // Main send loop
    let mut tick: u32 = 0;

    loop {
        tick += 1;

        // Stop heartbeats after some time in fault mode
        if simulate_fault && tick > 8 {
            println!("[{}] *** Simulating crash — going silent ***", device_id);
            thread::sleep(Duration::from_secs(30));
            continue;
        }

        // Build fake telemetry payload
        let temp = 20.0 + (tick as f32 * 0.3) % 15.0;
        let payload = format!("temp={:.1}C rpm={}", temp, 1000 + (tick * 7) % 500);

        // Send telemetry every second
        let telemetry = Message::new(&device_id, MsgType::Telemetry, &payload);
        send_message(&write_stream, &telemetry);

        // Send heartbeat every 2 ticks
        if tick % 2 == 0 {
            let hb = Message::new(&device_id, MsgType::Heartbeat, "alive");
            send_message(&write_stream, &hb);
        }

        thread::sleep(Duration::from_secs(1));
    }
}

// Write one JSON message to broker
fn send_message(stream: &Arc<Mutex<UnixStream>>, msg: &Message) {
    let json = serde_json::to_string(msg).unwrap();

    let mut s = stream.lock().unwrap();

    if let Err(e) = writeln!(s, "{}", json) {
        eprintln!("Send error: {}", e);
        std::process::exit(1);
    }
}
