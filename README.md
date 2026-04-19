````md
IPC Broker Daemon

This project is a simple Linux IPC system written in Rust. It has a central broker process, one or more device processes, and a monitor process. Devices send telemetry and heartbeat messages to the broker, and the broker forwards them to the monitor. The monitor can also send commands back to a selected device.

A small C client is also included to show that the same socket protocol works from both Rust and C.

## Main idea

The broker listens on a Unix domain socket:

```bash
/tmp/broker.sock
````

All clients connect to this socket and exchange newline-delimited JSON messages.

The communication flow is:

```text
device -> broker -> monitor
monitor -> broker -> device
```

## Main files

```text
src/
├── message.rs
└── bin/
    ├── broker.rs
    ├── device.rs
    └── monitor.rs

c_client/
├── device_c.c
└── Makefile
```

* `message.rs` defines the shared message format
* `broker.rs` handles client connections, message routing, and device health tracking
* `device.rs` simulates a Rust device that sends telemetry and receives commands
* `monitor.rs` displays messages and lets the user send commands
* `device_c.c` is a simple C client using the same socket protocol

## Message format

Each message is sent as JSON. Example:

```json
{
  "device_id": "device_1",
  "msg_type": "Telemetry",
  "timestamp": 1712345678,
  "payload": "temp=21.5C rpm=1035"
}
```

The main message types are:

* `Telemetry`
* `Heartbeat`
* `Fault`
* `Command`

## What the broker does

The broker accepts connections from devices and monitor clients, reads incoming messages, forwards device data to monitor clients, and keeps track of the last heartbeat from each device.

It also supports command routing. For example, if the monitor sends:

```text
target=device_1 cmd=reboot
```

the broker forwards that command to `device_1`.

## Device health tracking

The broker keeps a simple health state for each device:

* `NOMINAL`
* `DEGRADED`
* `FAULTED`
* `RECOVERING`

If heartbeats stop for too long, the state changes. If heartbeats start again, the device returns to normal.

## Build

From the project root:

```bash
cargo build
make -C c_client
```

## Run

Use separate terminals.

### Terminal 1

```bash
cargo run --bin broker
```

### Terminal 2

```bash
cargo run --bin device -- device_1
```

### Terminal 3

```bash
cargo run --bin monitor
```

### Terminal 4

```bash
./c_client/device_c c_sensor_1
```

## Send a command

In the monitor terminal, type:

```bash
cmd device_1 reboot
```

The broker will route the command and the device will print that it received it.

## Optional test

To simulate a device fault:

```bash
cargo run --bin device -- device_2 --fault
```

This stops heartbeats after a few seconds so the broker can detect that the device has gone silent.

```

