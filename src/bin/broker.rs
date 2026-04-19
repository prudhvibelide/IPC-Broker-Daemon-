/* Author: Prudhvi Raj Belide
   Sept 2025
   File : Central IPC broker for routing messages and tracking device health */

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Bring shared message types
#[path = "../message.rs"]
mod message;
use message::{Message, MsgType};

// Device health states
#[derive(Debug, Clone, PartialEq)]
enum DeviceState {
    Nominal,
    Degraded { missed: u32 },
    Faulted,
    Recovering,
}

impl std::fmt::Display for DeviceState {
    // Print state in readable format
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceState::Nominal => write!(f, "NOMINAL"),
            DeviceState::Degraded { missed } => write!(f, "DEGRADED(missed={})", missed),
            DeviceState::Faulted => write!(f, "FAULTED"),
            DeviceState::Recovering => write!(f, "RECOVERING"),
        }
    }
}

// Per-device health info
#[derive(Debug)]
struct DeviceHealth {
    state: DeviceState,
    last_heartbeat: Instant,
    fault_count: u32,
}

impl DeviceHealth {
    // Default healthy entry
    fn new() -> Self {
        DeviceHealth {
            state: DeviceState::Nominal,
            last_heartbeat: Instant::now(),
            fault_count: 0,
        }
    }
}

// Shared broker data
struct BrokerState {
    // Monitor sockets for broadcasting
    monitors: Vec<UnixStream>,

    // Device sockets for direct commands
    device_sockets: HashMap<String, UnixStream>,

    // Health table per device
    health: HashMap<String, DeviceHealth>,

    // Count of routed messages
    msg_count: u64,
}

impl BrokerState {
    // Create empty broker state
    fn new() -> Self {
        BrokerState {
            monitors: Vec::new(),
            device_sockets: HashMap::new(),
            health: HashMap::new(),
            msg_count: 0,
        }
    }

    // Update health when device message arrives
    fn record_message(&mut self, device_id: &str, msg_type: &MsgType) {
        let health = self
            .health
            .entry(device_id.to_string())
            .or_insert_with(DeviceHealth::new);

        if *msg_type == MsgType::Heartbeat {
            health.last_heartbeat = Instant::now();

            // Move back to healthy on heartbeat
            if health.state != DeviceState::Nominal {
                println!(
                    "[FDIR] {} recovered → NOMINAL (was {})",
                    device_id, health.state
                );
                health.state = DeviceState::Nominal;
            }
        }

        self.msg_count += 1;
    }

    // Run watchdog on all devices
    fn run_watchdog(&mut self) {
        let now = Instant::now();

        for (id, health) in self.health.iter_mut() {
            let elapsed = now.duration_since(health.last_heartbeat).as_secs();

            match &health.state {
                DeviceState::Nominal => {
                    if elapsed >= 3 {
                        health.state = DeviceState::Degraded { missed: 1 };
                        println!("[FDIR] {} → DEGRADED (no heartbeat for {}s)", id, elapsed);
                    }
                }
                DeviceState::Degraded { missed } => {
                    let missed = *missed;

                    if elapsed >= 6 {
                        health.fault_count += 1;
                        health.state = DeviceState::Faulted;
                        println!(
                            "[FDIR] {} → FAULTED (fault_count={})",
                            id, health.fault_count
                        );
                    } else {
                        health.state = DeviceState::Degraded { missed: missed + 1 };
                    }
                }
                DeviceState::Faulted => {
                    // Move to recovery wait state
                    health.state = DeviceState::Recovering;
                    println!("[FDIR] {} → RECOVERING (waiting for device to reconnect)", id);
                }
                DeviceState::Recovering => {
                    // Stay here until heartbeat comes back
                }
            }
        }
    }

    // Send message to all monitors
    fn broadcast_to_monitors(&mut self, line: &str) {
        self.monitors.retain_mut(|stream| writeln!(stream, "{}", line).is_ok());
    }

    // Print current broker state
    fn print_status(&self) {
        println!("=== Broker Status ===");
        println!("  Messages routed : {}", self.msg_count);
        println!("  Monitors connected : {}", self.monitors.len());

        for (id, h) in &self.health {
            println!("  Device {:12} : {}", id, h.state);
        }

        println!("=====================");
    }
}

fn main() {
    // Main broker socket path
    let broker_sock = "/tmp/broker.sock";

    // Extra socket path placeholder
    let status_sock = "/tmp/broker_status.sock";

    // Remove old sockets from previous run
    let _ = std::fs::remove_file(broker_sock);
    let _ = std::fs::remove_file(status_sock);

    println!("[broker] Starting IPC Broker Daemon");
    println!("[broker] Listening on {}", broker_sock);

    // Shared state across threads
    let state = Arc::new(Mutex::new(BrokerState::new()));

    // Watchdog thread for heartbeat checks
    {
        let state = Arc::clone(&state);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(1));
            state.lock().unwrap().run_watchdog();
        });
    }

    // Periodic status print thread
    {
        let state = Arc::clone(&state);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            state.lock().unwrap().print_status();
        });
    }

    // Listen for new client connections
    let listener = UnixListener::bind(broker_sock).expect("Failed to bind broker socket");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_client(stream, state));
            }
            Err(e) => eprintln!("[broker] Accept error: {}", e),
        }
    }
}

// Handle one connected client
fn handle_client(stream: UnixStream, state: Arc<Mutex<BrokerState>>) {
    // Separate write handle for routing
    let write_stream = stream.try_clone().expect("Failed to clone stream");

    // Line-based socket reader
    let reader = BufReader::new(stream);

    // Remember which device owns this socket
    let mut my_device_id: Option<String> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        match serde_json::from_str::<Message>(&line) {
            Ok(msg) => {
                let mut s = state.lock().unwrap();

                // Register monitor client
                if msg.device_id == "monitor"
                    && msg.msg_type == MsgType::Command
                    && msg.payload == "register"
                {
                    println!("[broker] Monitor connected");
                    s.monitors.push(write_stream.try_clone().unwrap());
                    continue;
                }

                // Route command to target device
                if msg.msg_type == MsgType::Command {
                    let target = msg
                        .payload
                        .split_whitespace()
                        .find(|p| p.starts_with("target="))
                        .and_then(|p| p.strip_prefix("target="))
                        .map(|s| s.to_string());

                    if let Some(target_id) = target {
                        if let Some(dev_sock) = s.device_sockets.get_mut(&target_id) {
                            println!("[broker] Command → {} │ {}", target_id, msg.payload);
                            let _ = writeln!(dev_sock, "{}", line);
                        } else {
                            println!("[broker] Command for unknown device: {}", target_id);
                        }
                    }

                    // Show command on monitor side too
                    s.broadcast_to_monitors(&line);
                    continue;
                }

                // Register first normal device message
                if my_device_id.is_none() {
                    println!("[broker] Device '{}' registered", msg.device_id);
                    s.device_sockets
                        .insert(msg.device_id.clone(), write_stream.try_clone().unwrap());
                    my_device_id = Some(msg.device_id.clone());
                }

                println!(
                    "[broker] {} │ {:?} │ {}",
                    msg.device_id, msg.msg_type, msg.payload
                );

                // Update heartbeat tracking
                s.record_message(&msg.device_id, &msg.msg_type);

                // Forward to all monitor clients
                s.broadcast_to_monitors(&line);
            }
            Err(e) => {
                eprintln!("[broker] Bad message: {} — {}", line, e);
            }
        }
    }

    // Remove device socket on disconnect
    if let Some(id) = my_device_id {
        state.lock().unwrap().device_sockets.remove(&id);
        println!("[broker] Device '{}' disconnected", id);
    } else {
        println!("[broker] Client disconnected");
    }
}
