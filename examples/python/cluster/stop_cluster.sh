#!/bin/bash

echo "Stopping all cluster processes..."

killall -9 director worker client 2>/dev/null
pkill -9 -f "target.*debug.*(director|worker|client)" 2>/dev/null
pkill -9 -f "cargo run.*director" 2>/dev/null
pkill -9 -f "cargo run.*worker" 2>/dev/null
pkill -9 -f "cargo run.*client" 2>/dev/null

echo "✅ All cluster processes stopped"
