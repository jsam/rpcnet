#!/bin/bash
# Helper script to run the director from the Python examples directory

cd "$(dirname "$0")/../../.."

DIRECTOR_ADDR=${DIRECTOR_ADDR:-127.0.0.1:61000}
RUST_LOG=${RUST_LOG:-info}

echo "🎯 Starting Director at $DIRECTOR_ADDR"
echo "📂 Working directory: $(pwd)"

DIRECTOR_ADDR=$DIRECTOR_ADDR RUST_LOG=$RUST_LOG \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director --release
