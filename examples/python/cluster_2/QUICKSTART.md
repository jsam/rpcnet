# Python Worker with SWIM Cluster - Quick Start Guide

This guide demonstrates how to run a Python inference worker that integrates with RpcNet's SWIM cluster, alongside the Rust director and clients.

## Overview

The cluster consists of three components:

1. **Director** (Rust) - Coordinates the cluster and routes requests to workers
2. **Python Worker** - Handles inference requests, participates in SWIM gossip
3. **Client** (Rust or Python) - Sends inference requests through the director

```
┌─────────────┐
│   Client    │────┐
│ (Rust/Py)   │    │
└─────────────┘    │
                   ▼
            ┌──────────────┐         ┌─────────────────┐
            │   Director   │◄───────►│ Python Worker   │
            │   (Rust)     │  SWIM   │   (Python)      │
            └──────────────┘         └─────────────────┘
                   │
                   ├─► Routes requests
                   ├─► Load balancing
                   └─► Failure detection
```

## Prerequisites

### 1. Build the Python Extension Module

From the project root:

```bash
maturin develop --features extension-module
```

### 2. Generate Python Bindings

```bash
cargo build --bin rpcnet-gen --features codegen,python --release
./target/release/rpcnet-gen \
  --input examples/python/cluster_2/inference.rpc.rs \
  --output examples/python/cluster_2/generated \
  --python
```

### 3. Generate TLS Certificates

```bash
mkdir -p certs
cd certs
openssl req -x509 -newkey rsa:4096 \
  -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
cd ..
```

### 4. Build the Cluster Example (Rust components)

```bash
cd examples/cluster
cargo build --release
cd ../..
```

## Running the Cluster

### Terminal 1: Start the Director

The director coordinates the cluster and routes requests to workers.

```bash
cd examples/cluster
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin director --release
```

**Expected output:**
```
🎯 Starting Director at 127.0.0.1:61000
📁 Loading certificates from "certs/test_cert.pem" and "certs/test_key.pem"
RPC server listening on 127.0.0.1:61000
✅ Director registered itself in cluster
✅ Cluster enabled - Director is now discoverable
🔄 Load balancing strategy: LeastConnections
🚀 Director ready - listening on 127.0.0.1:61000
⚠️  No workers available
```

### Terminal 2: Start the Python Worker

The Python worker joins the SWIM cluster and handles inference requests.

```bash
cd examples/python/cluster_2

WORKER_LABEL=python-worker \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=../../../certs/test_cert.pem \
  KEY_PATH=../../../certs/test_key.pem \
  ../../../.venv/bin/python python_worker.py
```

**Expected output:**
```
======================================================================
🐍 Python Inference Worker with SWIM Cluster
======================================================================
Worker Label: python-worker
Worker Address: 127.0.0.1:62002
Director Address: 127.0.0.1:61000
Certificate: ../../../certs/test_cert.pem
Key: ../../../certs/test_key.pem
======================================================================

🔌 Binding server to 127.0.0.1:62002...
✅ Server bound
🌐 Creating QUIC client...
✅ QUIC client created
🔗 Enabling cluster, connecting to director at 127.0.0.1:61000...
✅ Cluster enabled
🏷️  Updating cluster tags...
✅ Tags updated
🚀 Python worker on 127.0.0.1:62002 is now ready!
✅ Starting to serve inference requests...
💡 Press Ctrl+C to stop
```

**Director should now show:**
```
📊 Worker pool status: 1 workers available
   - node-127.0.0.1:62002 at 127.0.0.1:62002 (0 connections)
```

### Terminal 3: Run the Rust Client

Send inference requests through the director.

```bash
cd examples/cluster
DIRECTOR_ADDR=127.0.0.1:61000 \
  cargo run --bin client --release
```

**Expected output:**
```
Connecting to director at 127.0.0.1:61000...
Sending inference request: "Hello from Rust client"
Response: Connected
Response: Token { text: "Python worker 'python-worker' processed: Hello from Rust client", sequence: 1 }
Response: Done
✅ Request completed successfully
```

**Python worker logs:**
```
📥 Received inference request #1
   Connection ID: conn-abc123
   Prompt: Hello from Rust client
📤 Sending response: Python worker 'python-worker' processed: Hello from Rust client...
```

### Terminal 4 (Optional): Run the Python Client

You can also send requests using a Python client.

```bash
cd examples/python/cluster_2

DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=../../../certs/test_cert.pem \
  ../../../.venv/bin/python test_client.py
```

## What's Happening?

### 1. SWIM Cluster Integration

The Python worker:
- ✅ Joins the SWIM gossip cluster by connecting to the director
- ✅ Participates in failure detection via heartbeats
- ✅ Registers with tags: `role=worker`, `label=python-worker`, `language=python`
- ✅ Appears in the director's worker pool
- ✅ Automatically gets discovered by the director for request routing

### 2. Request Flow

```
Client → Director → Python Worker → Director → Client
  1. Client sends inference request to director
  2. Director routes to Python worker (load balanced)
  3. Python worker processes request
  4. Response flows back through director to client
```

### 3. Load Balancing

The director uses **Least Connections** strategy:
- Tracks active connections per worker
- Routes new requests to the worker with fewest connections
- Ensures even distribution of load

### 4. Failure Detection

If the Python worker crashes or becomes unresponsive:
- SWIM gossip protocol detects the failure (within ~5-10 seconds)
- Director marks the worker as failed
- Director stops routing requests to the failed worker
- When worker recovers, it's automatically re-discovered

## Testing Failure Scenarios

### Test 1: Stop the Python Worker

1. Press `Ctrl+C` in the Python worker terminal
2. Observe director logs:
   ```
   ⚠️  Worker node-127.0.0.1:62002 marked as failed
   ⚠️  No workers available
   ```
3. Restart the Python worker
4. Director should show:
   ```
   ✅ Worker node-127.0.0.1:62002 recovered
   📊 Worker pool status: 1 workers available
   ```

### Test 2: Multiple Workers

Start a second Python worker on a different port:

```bash
WORKER_LABEL=python-worker-2 \
  WORKER_ADDR=127.0.0.1:62003 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=../../../certs/test_cert.pem \
  KEY_PATH=../../../certs/test_key.pem \
  ../../../.venv/bin/python python_worker.py
```

Director should show:
```
📊 Worker pool status: 2 workers available
   - node-127.0.0.1:62002 at 127.0.0.1:62002 (0 connections)
   - node-127.0.0.1:62003 at 127.0.0.1:62003 (0 connections)
```

Requests will be load-balanced between both workers!

## Environment Variables

### Director
- `DIRECTOR_ADDR` - Address to bind (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

### Python Worker
- `WORKER_LABEL` - Unique label for the worker (default: `python-worker`)
- `WORKER_ADDR` - Address to bind (default: `127.0.0.1:62002`)
- `DIRECTOR_ADDR` - Director address to connect to (default: `127.0.0.1:61000`)
- `CERT_PATH` - Path to TLS certificate (default: `certs/test_cert.pem`)
- `KEY_PATH` - Path to TLS key (default: `certs/test_key.pem`)

### Client
- `DIRECTOR_ADDR` - Director address to connect to (default: `127.0.0.1:61000`)
- `CERT_PATH` - Path to TLS certificate

## Architecture Details

### SWIM Gossip Protocol

The Python worker participates in SWIM (Scalable Weakly-consistent Infection-style Process Group Membership):

1. **Heartbeats**: Workers send periodic pings to director
2. **Failure Detection**: Uses Phi Accrual algorithm for adaptive detection
3. **Gossip**: Membership changes propagate via gossip messages
4. **Conflict Resolution**: Incarnation numbers resolve conflicting states

### Python-Rust Integration

The Python worker uses PyO3 bindings to:
- Call Rust's QUIC/TLS implementation
- Participate in SWIM cluster
- Handle RPC requests with Python async/await
- Serialize/deserialize with MessagePack

### Generated Code

The `generated/inference/` directory contains:
- `types.py` - Request/Response dataclasses with enum support
- `server.py` - Server wrapper with handler registration
- `client.py` - Type-safe client with async methods

## Troubleshooting

### "Address already in use"
- Another process is using the port
- Kill the process: `lsof -ti:62002 | xargs kill -9`

### "Connection refused"
- Director is not running
- Check `DIRECTOR_ADDR` matches in both worker and director

### "ModuleNotFoundError: No module named '_rpcnet'"
- Run `maturin develop --features extension-module` from project root

### "TLS error"
- Regenerate certificates (see Prerequisites step 3)
- Ensure `CERT_PATH` and `KEY_PATH` point to valid files

### Worker not appearing in cluster
- Check director logs for connection errors
- Verify network connectivity between worker and director
- Ensure both are using the same certificates

## References

- [RpcNet Documentation](https://jsam.github.io/rpcnet/)
- [SWIM Protocol Paper](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf)
- [PyO3 Documentation](https://pyo3.rs/)
