/* Author: Prudhvi Raj Belide
   Sept 2025
   File : Shared message types for broker, devices, and monitor */

// Different message kinds used in system
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum MsgType {
    Telemetry,
    Heartbeat,
    Fault,
    Command,
}

// Common message frame sent over socket
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    // Sender name like device_1 or monitor
    pub device_id: String,

    // Message category
    pub msg_type: MsgType,

    // Unix time in seconds
    pub timestamp: u64,

    // Actual data string
    pub payload: String,
}

impl Message {
    // Build a new message with current timestamp
    pub fn new(device_id: &str, msg_type: MsgType, payload: &str) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Message {
            device_id: device_id.to_string(),
            msg_type,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: payload.to_string(),
        }
    }
}
