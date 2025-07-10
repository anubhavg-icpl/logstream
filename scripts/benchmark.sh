#!/bin/bash
# LogStream Performance Benchmark Script

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[BENCHMARK]${NC} $1"
}

# Configuration
SOCKET_PATH="/tmp/logstream_benchmark.sock"
LOG_DIR="/tmp/logstream_benchmark"
NUM_MESSAGES=10000
NUM_CLIENTS=10

# Cleanup function
cleanup() {
    print_status "Cleaning up..."
    pkill -f "logstream-server" || true
    rm -f "$SOCKET_PATH"
    rm -rf "$LOG_DIR"
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Create directories
mkdir -p "$LOG_DIR"

print_status "Building LogStream in release mode..."
cargo build --release

print_status "Starting LogStream server..."
./target/release/logstream-server -s "$SOCKET_PATH" -o "$LOG_DIR" &
SERVER_PID=$!
sleep 2

print_status "Running benchmark with $NUM_CLIENTS clients sending $NUM_MESSAGES messages each..."

# Run client benchmark
time {
    for i in $(seq 1 $NUM_CLIENTS); do
        (
            cargo run --release --example client_example -- \
                --socket "$SOCKET_PATH" \
                --daemon "benchmark-client-$i" \
                --messages $NUM_MESSAGES
        ) &
    done
    wait
}

# Calculate statistics
TOTAL_MESSAGES=$((NUM_CLIENTS * NUM_MESSAGES))
print_status "Total messages sent: $TOTAL_MESSAGES"

# Check log files
print_status "Log files created:"
ls -la "$LOG_DIR"/*.log | wc -l

print_status "Total log size:"
du -sh "$LOG_DIR"

echo -e "${GREEN}Benchmark completed!${NC}"