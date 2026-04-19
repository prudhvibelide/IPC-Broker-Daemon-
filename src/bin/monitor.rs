/* Author: Prudhvi Raj Belide
   Sept 2025
   File : Monitor client for viewing traffic and sending commands */

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::thread;

// Bring shared message types
#[path = "../message.rs"]
mod message;
use message::{Message, MsgType};

// Terminal colors for cleaner output
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";

fn main() {
    println!("[monitor] Connecting to broker...");

    // Connect to broker socket
    let stream =
        UnixStream::connect("/tmp/broker.sock").expect("Cannot connect to broker. Is it running?");

    // Keep write side for commands
    let mut write_stream = stream.try_clone().unwrap();

    // Register this client as monitor
    let reg = Message::new("monitor", MsgType::Command, "register");
    let json = serde_json::to_string(&reg).unwrap();
    writeln!(write_stream, "{}", json).unwrap();

    println!("[monitor] Registered. Waiting for messages...");
    println!(
        "{}Tip: type 'cmd <device_id> <command>' to send a command{}\n",
        BOLD, RESET
    );
    println!("{:<12} {:<12} {}", "DEVICE", "TYPE", "PAYLOAD");
    println!("{}", "─".repeat(55));

    // Reader thread for incoming messages
    thread::spawn(move || {
        let reader = BufReader::new(stream);

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            match serde_json::from_str::<Message>(&line) {
                Ok(msg) => {
                    let (color, type_str) = match msg.msg_type {
                        MsgType::Telemetry => (GREEN, "TELEMETRY"),
                        MsgType::Heartbeat => (CYAN, "HEARTBEAT"),
                        MsgType::Fault => (RED, "FAULT    "),
                        MsgType::Command => (YELLOW, "COMMAND  "),
                    };

                    println!(
                        "{}{:<12}{} {}{}{} {}",
                        GREEN, msg.device_id, RESET, color, type_str, RESET, msg.payload
                    );
                }
                Err(e) => eprintln!("[monitor] Parse error: {}", e),
            }
        }
    });

    // Read user commands from stdin
    let stdin = std::io::stdin();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // Expected format: cmd <device_id> <command>
        let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();

        if parts.len() == 3 && parts[0] == "cmd" {
            let target = parts[1];
            let cmd = parts[2];

            // Encode target in payload string
            let payload = format!("target={} cmd={}", target, cmd);

            let msg = Message::new("monitor", MsgType::Command, &payload);
            let json = serde_json::to_string(&msg).unwrap();

            writeln!(write_stream, "{}", json).unwrap();
            println!(
                "{}[monitor] Sent command to {}: {}{}",
                YELLOW, target, cmd, RESET
            );
        } else {
            println!("Usage: cmd <device_id> <command>");
        }
    }
}
