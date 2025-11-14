#!/bin/bash
# Helper script to run the Rust client from the Python examples directory

cd "$(dirname "$0")/../../.."

DIRECTOR_ADDR=${DIRECTOR_ADDR:-127.0.0.1:61000}
RUST_LOG=${RUST_LOG:-info}

echo "🚀 Starting Rust Client"
echo "📍 Director at $DIRECTOR_ADDR"
echo "📂 Working directory: $(pwd)"

DIRECTOR_ADDR=$DIRECTOR_ADDR RUST_LOG=$RUST_LOG \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client --release
