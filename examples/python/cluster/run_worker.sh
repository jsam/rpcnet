#!/bin/bash
# Helper script to run a Rust worker from the Python examples directory

cd "$(dirname "$0")/../../.."

WORKER_LABEL=${WORKER_LABEL:-worker-1}
WORKER_ADDR=${WORKER_ADDR:-127.0.0.1:62001}
DIRECTOR_ADDR=${DIRECTOR_ADDR:-127.0.0.1:61000}
RUST_LOG=${RUST_LOG:-info}

echo "👷 Starting Worker '$WORKER_LABEL' at $WORKER_ADDR"
echo "📍 Director at $DIRECTOR_ADDR"
echo "📂 Working directory: $(pwd)"

WORKER_LABEL=$WORKER_LABEL WORKER_ADDR=$WORKER_ADDR \
  DIRECTOR_ADDR=$DIRECTOR_ADDR RUST_LOG=$RUST_LOG \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker --release
