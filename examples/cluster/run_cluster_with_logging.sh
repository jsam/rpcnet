#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

LOGFILE="cluster_combined.log"

echo "Starting cluster with unified logging to $LOGFILE"
echo "Press Ctrl+C to stop all processes"
echo ""

rm -f "$LOGFILE"
touch "$LOGFILE"

trap 'kill $(jobs -p) 2>/dev/null; echo "All processes stopped"; exit 0' SIGINT SIGTERM

(DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info cargo run --bin director 2>&1 | while IFS= read -r line; do echo "[$(date '+%Y-%m-%d %H:%M:%S.%3N')] [DIRECTOR] $line"; done | tee -a "$LOGFILE") &

sleep 3

(WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info cargo run --bin worker 2>&1 | while IFS= read -r line; do echo "[$(date '+%Y-%m-%d %H:%M:%S.%3N')] [WORKER-A] $line"; done | tee -a "$LOGFILE") &

(WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info cargo run --bin worker 2>&1 | while IFS= read -r line; do echo "[$(date '+%Y-%m-%d %H:%M:%S.%3N')] [WORKER-B] $line"; done | tee -a "$LOGFILE") &

sleep 6

(DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info cargo run --bin client 2>&1 | while IFS= read -r line; do echo "[$(date '+%Y-%m-%d %H:%M:%S.%3N')] [CLIENT  ] $line"; done | tee -a "$LOGFILE") &

echo "All processes started. Logging to $LOGFILE"
echo "You can tail the log with: tail -f $LOGFILE"
echo ""

wait
