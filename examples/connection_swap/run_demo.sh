#!/bin/bash

set -e

echo "=== Connection Swap Demo ==="
echo ""
echo "This demo shows seamless QUIC connection migration between workers."
echo "The client will connect to a director, which assigns workers."
echo "Worker-a will serve for ~15 seconds, then simulate a failure."
echo "The director will automatically switch to worker-b without dropping the connection."
echo ""

cd "$(dirname "$0")"

cleanup() {
    echo ""
    echo "Cleaning up processes..."
    pkill -f "connection_swap.*director" || true
    pkill -f "connection_swap.*worker" || true
    sleep 1
}

trap cleanup EXIT

cleanup

echo "Starting director (dual-port: USER=61000, MGMT=61001)..."
CONNECTION_SWAP_DIRECTOR_USER_ADDR=127.0.0.1:61000 \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path Cargo.toml --bin director &

sleep 2

echo "Starting worker-a (dual-port: USER=62001, MGMT=63001)..."
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path Cargo.toml --bin worker &

sleep 1

echo "Starting worker-b (dual-port: USER=62002, MGMT=63002)..."
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path Cargo.toml --bin worker &

sleep 2

echo ""
echo "All components ready. Starting client..."
echo "=========================================="
echo ""

CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 RUST_LOG=info \
    cargo run --manifest-path Cargo.toml --bin client

echo ""
echo "Demo completed!"
