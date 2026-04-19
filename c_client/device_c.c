/* Author: Prudhvi Raj Belide
   Sept 2025
   File : C device client for sending telemetry to the Rust broker */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <sys/socket.h>
#include <sys/un.h>

#define BROKER_SOCK "/tmp/broker.sock"
#define BUF_SIZE 256

// Open Unix socket connection to broker
static int connect_to_broker(void) {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        perror("socket");
        exit(1);
    }

    // Fill socket address
    struct sockaddr_un addr = {0};
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, BROKER_SOCK, sizeof(addr.sun_path) - 1);

    // Connect to running broker
    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        perror("connect - is the broker running?");
        exit(1);
    }

    return fd;
}

// Send one JSON message line
static void send_msg(int fd, const char *device_id,
                     const char *msg_type, const char *payload) {
    char buf[BUF_SIZE];
    unsigned long ts = (unsigned long)time(NULL);

    int n = snprintf(buf, sizeof(buf),
        "{\"device_id\":\"%s\",\"msg_type\":\"%s\","
        "\"timestamp\":%lu,\"payload\":\"%s\"}\n",
        device_id, msg_type, ts, payload);

    write(fd, buf, n);
}

int main(int argc, char *argv[]) {
    // Use given device name or default
    const char *device_id = (argc > 1) ? argv[1] : "c_sensor_1";

    printf("[%s] Connecting to broker...\n", device_id);

    int fd = connect_to_broker();

    printf("[%s] Connected. Sending telemetry...\n", device_id);

    for (int tick = 1; tick <= 20; tick++) {
        // Build simple telemetry string
        char payload[64];
        float temp = 18.0f + (tick % 12) * 0.5f;
        snprintf(payload, sizeof(payload), "temp=%.1fC voltage=3.%d", temp, tick % 10);

        // Send telemetry each tick
        send_msg(fd, device_id, "Telemetry", payload);
        printf("[%s] sent telemetry: %s\n", device_id, payload);

        // Send heartbeat every 2 ticks
        if (tick % 2 == 0) {
            send_msg(fd, device_id, "Heartbeat", "alive");
            printf("[%s] sent heartbeat\n", device_id);
        }

        sleep(1);
    }

    printf("[%s] Done.\n", device_id);

    close(fd);
    return 0;
}
