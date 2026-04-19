#!/bin/bash
set -e

PASS=0
FAIL=0

pass() { echo "PASS: $1"; PASS=$((PASS+1)); }
fail() { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

cleanup() {
  pkill -f "target/debug/broker" 2>/dev/null || true
  pkill -f "target/debug/device" 2>/dev/null || true
  pkill -f "target/debug/monitor" 2>/dev/null || true
  pkill -f "./c_client/device_c" 2>/dev/null || true
  rm -f /tmp/broker.sock
}
trap cleanup EXIT

cleanup

echo "[1] Build"
cargo build >/tmp/test_build.log 2>&1 || { cat /tmp/test_build.log; exit 1; }
make -C c_client >/tmp/test_c_build.log 2>&1 || { cat /tmp/test_c_build.log; exit 1; }
pass "Build successful"

echo "[2] Start broker"
./target/debug/broker >/tmp/broker.log 2>&1 &
sleep 1
[ -S /tmp/broker.sock ] && pass "Broker socket created" || fail "Broker socket missing"

echo "[3] Start device_1"
./target/debug/device device_1 >/tmp/device1.log 2>&1 &
sleep 2
grep -q "Device 'device_1' registered" /tmp/broker.log && pass "device_1 registered" || fail "device_1 not registered"

echo "[4] Start monitor"
(
  sleep 2
  echo "cmd device_1 reboot"
  sleep 2
) | ./target/debug/monitor >/tmp/monitor.log 2>&1 &
sleep 5

grep -q "Monitor connected" /tmp/broker.log && pass "Monitor connected" || fail "Monitor not connected"
grep -q "Command → device_1" /tmp/broker.log && pass "Command routed to device_1" || fail "Command not routed"
grep -q "COMMAND RECEIVED" /tmp/device1.log && pass "device_1 received command" || fail "device_1 did not receive command"

echo "[5] Start C client"
./c_client/device_c c_sensor_1 >/tmp/c_client.log 2>&1 &
sleep 4
grep -q "Device 'c_sensor_1' registered" /tmp/broker.log && pass "C client registered" || fail "C client not registered"
grep -q "c_sensor_1 │ Telemetry" /tmp/broker.log && pass "C telemetry received" || fail "C telemetry missing"
grep -q "c_sensor_1 │ Heartbeat" /tmp/broker.log && pass "C heartbeat received" || fail "C heartbeat missing"

echo
echo "Total Passed: $PASS"
echo "Total Failed: $FAIL"

if [ "$FAIL" -eq 0 ]; then
  echo "ALL TESTS PASSED"
  exit 0
else
  echo "SOME TESTS FAILED"
  exit 1
fi
