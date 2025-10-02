# Connection Swap Example - Quick Start

This example demonstrates seamless QUIC connection migration between workers.

## What This Demo Shows

A fully functional multi-process demonstration of:

1. **Director** - Routes client connections to workers
2. **Workers** - Handle actual work, simulate failures
3. **Client** - Issues long-running requests, observes seamless failover

## Running the Demo

### Option 1: Automated Demo Script (Recommended)

```bash
cd examples/connection_swap
./run_demo.sh
```

This script will:
- Start the director with dual ports (USER: 61000, MGMT: 61001)
- Start worker-a with dual ports (USER: 62001, MGMT: 63001)
- Start worker-b with dual ports (USER: 62002, MGMT: 63002)
- Run the client to demonstrate connection migration
- Clean up all processes when done

### Option 2: Manual Setup

Terminal 1 - Director (dual-port):
```bash
cd examples/connection_swap
CONNECTION_SWAP_DIRECTOR_USER_ADDR=127.0.0.1:61000 \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --bin director
```

Terminal 2 - Worker A (dual-port):
```bash
cd examples/connection_swap
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --bin worker
```

Terminal 3 - Worker B (dual-port):
```bash
cd examples/connection_swap
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --bin worker
```

Terminal 4 - Client:
```bash
cd examples/connection_swap
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 RUST_LOG=info \
    cargo run --bin client
```

## What You'll See

### 1. Initial Connection

```
INFO connection.id=conn-1234 worker=worker-a stream.id=1: 🔄 worker assigned to stream
INFO connection.id=conn-1234 worker=worker-a stream.id=1 sequence=1: 📦 received token
INFO connection.id=conn-1234 worker=worker-a stream.id=1 sequence=2: 📦 received token
...
```

### 2. Worker-a Failure (after ~15 seconds)

```
Worker-a:
WARN worker=worker-a connection.id=conn-1234: ⚠️  simulating failure after 15s

Director:
WARN stream.id=1 connection.id=conn-1234 worker=worker-a: ⚠️  worker failed - initiating connection migration
INFO stream.id=1 connection.id=conn-1234 from_worker=worker-a to_worker=worker-b: 🔀 migrating connection to new worker

Client:
INFO connection.id=conn-1234 from_worker=worker-a to_worker=worker-b: 🔀 CONNECTION MIGRATION: switching workers
```

### 3. Automatic Migration to Worker-b

```
INFO connection.id=conn-1234 worker=worker-b stream.id=1: 🔄 worker assigned to stream  
INFO connection.id=conn-1234 worker=worker-b stream.id=1 sequence=31: 📦 received token
INFO connection.id=conn-1234 worker=worker-b stream.id=1 sequence=32: 📦 received token
...
```

### 4. Key Innovation: Same Connection ID

Notice that the `connection.id` **remains constant** throughout:
```
connection.id=conn-1234 worker=worker-a  # Before migration
connection.id=conn-1234 worker=worker-b  # After migration (same ID!)
```

This proves the QUIC connection was migrated, not recreated.

## Architecture

```
┌─────────┐
│ Client  │
└────┬────┘
     │ QUIC (USER port 61000)
     ↓
┌─────────────────┐
│    Director     │←─── Worker-a
│ USER:  61000    │      USER:  62001, MGMT: 63001
│ MGMT:  61001    │
└─────────────────┘←─── Worker-b
                        USER:  62002, MGMT: 63002
```

### Dual-Port Architecture

Each component runs two servers:
- **USER Port**: Business logic RPC endpoints (client requests, streaming)
- **MGMT Port**: Control plane (health checks, worker registration, migration coordination)

### Migration Flow

```
Client connects to Director
    ↓
Director assigns to Worker-A
    ↓
Worker-A serves ~15 seconds (tokens 1-30)
    ↓
Worker-A simulates failure
    ↓
Director detects failure
    ↓
Director switches connection to Worker-B
    ↓
Worker-B continues (tokens 31+)
    ↓
Client receives uninterrupted stream (same connection!)
```

## Implementation Details

### Director (`src/bin/director.rs`)
- **USER Port (61000)**: Client streaming requests (`generate`)
- **MGMT Port (61001)**: Worker registration, health checks
- Manages worker pool with round-robin assignment
- Automatically retries with next worker on failure
- Sends migration notifications to clients
- Maintains the same connection_id throughout migration

### Worker (`src/bin/worker.rs`)
- **USER Port (62001/62002)**: Streaming inference endpoint (`generate`)
- **MGMT Port (63001/63002)**: Health check endpoint
- Registers with director's MGMT port (heartbeats every 5s)
- Generates tokens at 500ms intervals
- Simulates failure after 15 seconds
- Each worker has unique label (worker-a, worker-b, etc.)

### Client (`src/bin/client.rs`)
- Connects to director
- Issues streaming inference request
- Receives tokens from workers
- Observes worker transitions seamlessly
- Logs connection_id to prove continuity

## Core Migration Infrastructure

This example builds on the complete migration infrastructure in `src/migration/`:

✅ **MigrationStateMachine** - State management (166 tests passing)  
✅ **ConnectionSessionManager** - Session tracking  
✅ **MigrationServiceImpl** - Complete service implementation  
✅ **Token-based authentication** - Cryptographic validation  
✅ **State transfer services** - Connection state serialization  

See [`IMPLEMENTED.md`](./IMPLEMENTED.md) for full documentation of the migration library.

## Troubleshooting

### "Connection refused"
Make sure the director is running before starting workers and client.

### "Address already in use"
Ports 61000-61001 (director), 62001-62002 (worker USER), 63001-63002 (worker MGMT) must be available. Kill existing processes:
```bash
pkill -f connection_swap
```

### TLS errors
Make sure you have valid certificates in `certs/`:
```bash
ls -la ../../certs/
```

You should see `cert.pem` and `key.pem`.

## Next Steps

This example demonstrates the **application-level orchestration** using the core migration primitives.

To extend this:
1. Add health checks for proactive migration
2. Implement load balancing policies
3. Add metrics and monitoring
4. Support multiple concurrent clients
5. Implement graceful worker shutdown

The migration infrastructure is production-ready!
