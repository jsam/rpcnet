#!/bin/bash

pkill -9 -f "cargo run.*director"
pkill -9 -f "cargo run.*worker"
pkill -9 -f "cargo run.*client"
sleep 2

echo "Starting director..."
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info cargo run --bin director 2>&1 | sed 's/^/[DIRECTOR] /' &
DIRECTOR_PID=$!

sleep 3

echo "Starting worker-a (with failure enabled)..."
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info cargo run --bin worker 2>&1 | sed 's/^/[WORKER-A] /' &
WORKER_A_PID=$!

sleep 2

echo "Starting worker-b (without failure)..."
WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=false RUST_LOG=info cargo run --bin worker 2>&1 | sed 's/^/[WORKER-B] /' &
WORKER_B_PID=$!

sleep 3

echo "Starting client..."
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info cargo run --bin client 2>&1 | sed 's/^/[CLIENT] /'

echo "Test complete. Press Ctrl+C to stop all processes."

wait
