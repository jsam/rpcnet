#!/bin/bash

# Kill any existing processes
pkill -9 -f "target/debug/director" 2>/dev/null
pkill -9 -f "target/debug/worker" 2>/dev/null
sleep 1

# Start director in background
echo "Starting director..."
DIRECTOR_ADDR=127.0.0.1:7000 RUST_LOG=cluster_example=info cargo run --bin director > /tmp/director_output.log 2>&1 &
DIRECTOR_PID=$!
echo "Director PID: $DIRECTOR_PID"

# Wait for director to start
sleep 5

# Start worker in background
echo "Starting worker..."
WORKER_LABEL=worker-1 WORKER_ADDR=127.0.0.1:7001 DIRECTOR_ADDR=127.0.0.1:7000 RUST_LOG=cluster_example=info cargo run --bin worker > /tmp/worker_output.log 2>&1 &
WORKER_PID=$!
echo "Worker PID: $WORKER_PID"

# Wait for worker to register
sleep 8

# Show logs
echo ""
echo "=== DIRECTOR LOG ==="
tail -30 /tmp/director_output.log

echo ""
echo "=== WORKER LOG ==="
tail -30 /tmp/worker_output.log

# Keep running for observation
echo ""
echo "Cluster is running. Press Ctrl+C to stop."
echo "Director PID: $DIRECTOR_PID"
echo "Worker PID: $WORKER_PID"

# Wait for user interrupt
trap "kill $DIRECTOR_PID $WORKER_PID 2>/dev/null; exit" INT
wait
