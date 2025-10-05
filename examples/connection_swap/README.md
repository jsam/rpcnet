# Connection Swap Example

**Worker failover with client-side reconnection** - A complete demonstration of automatic worker reassignment using rpcnet's streaming RPC infrastructure.

## 🚀 Quick Start

```bash
cd examples/connection_swap
./run_demo.sh
```

Watch as a client maintains its streaming session while the director automatically reassigns it from a failing worker to a healthy one!

## 📖 Documentation

- **[QUICKSTART.md](./QUICKSTART.md)** - Complete setup and usage guide
- **[ENVIRONMENT_VARIABLES.md](./ENVIRONMENT_VARIABLES.md)** - Configuration options

## 🎯 What This Demonstrates

This example shows a **fully functional** multi-process system demonstrating:

1. **Director Process** - Manages worker pool and assigns workers to clients
2. **Worker Processes** - Handle streaming requests, simulate failures  
3. **Client Process** - Issues long-running requests, reconnects on failure
4. **Automatic Failover** - Client detects failures and requests new worker assignment

### Key Pattern

The client maintains a logical session across multiple QUIC connections:

```
connection_id=conn-1234 worker=worker-a  ← Initial assignment (new QUIC connection)
connection_id=conn-1234 worker=worker-b  ← After failover (new QUIC connection to different worker)
```

The `connection_id` represents the **logical session**, while the underlying QUIC connections are created fresh for each worker assignment.

## 🏗️ Architecture

```
┌─────────┐
│ Client  │ ← Creates new QUIC connection on each reassignment
└────┬────┘
     │
     ↓ Asks for worker assignment
┌──────────────────┐     
│    Director      │←─── Worker-a (fails after 10-25s) 
│ USER:  61000     │      USER:  62001, MGMT: 63001
│ MGMT:  61001     │
└──────────────────┘←─── Worker-b (takes over)
                          USER:  62002, MGMT: 63002
```

### Dual-Port Architecture

Each component runs two servers:
- **USER Port**: RPC endpoints for business logic (client requests, streaming)
- **MGMT Port**: Control plane for health checks, worker registration

### Flow

1. Client connects to director and requests worker assignment
2. Director assigns worker-a and returns its address
3. Client establishes direct QUIC connection to worker-a
4. Worker-a serves for 10-25s (random), generating tokens
5. Worker-a simulates failure
6. Client detects failure via heartbeat (5s interval, 3s timeout)
7. Client goes back to director for new assignment
8. Director assigns worker-b
9. Client establishes new QUIC connection to worker-b
10. Worker-b continues generating tokens
11. Client receives uninterrupted logical session

## 🔧 Components

### Director (`src/bin/director.rs`)
- **USER Port (61000)**: Worker assignment requests (`get_worker`)
- **MGMT Port (61001)**: Worker registration, health checks
- Worker pool management with round-robin assignment
- Tracks worker availability (alive/recovering)
- Filters out unavailable workers from assignment

### Worker (`src/bin/worker.rs`)
- **USER Port (62001/62002)**: Streaming inference endpoint (`generate`), heartbeat
- **MGMT Port (63001/63002)**: Health check endpoint
- Registers with director's MGMT port on startup (heartbeats every 5s)
- Generates tokens at 500ms intervals
- Simulates failure after random interval (10-25s jitter)
- Marks self as unavailable during recovery (10s cooldown)
- Rejects new connections while unavailable

### Client (`src/bin/client.rs`)
- Connects to director to request worker assignment
- Establishes direct QUIC connection to assigned worker
- Issues streaming `generate` request
- Runs heartbeat task (5s interval, 3s timeout, 2 consecutive failures trigger reconnect)
- Detects worker failures and returns to director for reassignment
- Maintains `connection_id` across worker changes for session continuity

## 📊 Expected Output

### Initial Connection (Worker-a)
```
INFO connection.id=conn-abc123 worker=worker-a: 🔄 worker confirmed connection
INFO connection.id=conn-abc123 worker=worker-a sequence=1: 📦 received token
INFO connection.id=conn-abc123 worker=worker-a sequence=2: 📦 received token
...
INFO connection.id=conn-abc123 worker=worker-a: 💓 heartbeat ok
```

### Failure Event (~10-25s)
```
Worker-a:
WARN worker=worker-a connection.id=conn-abc123: ⚠️  simulating failure after 15s
INFO worker=worker-a: 💤 worker entering recovery mode (10s cooldown) - marked as unavailable

Client:
WARN connection.id=conn-abc123 worker=worker-a: 💔 heartbeat timeout
ERROR connection.id=conn-abc123 worker=worker-a: 💀 heartbeat failed 2 times - marking worker as failed
INFO 🔄 returning to director for new worker assignment
```

### Automatic Failover (Worker-b)
```
Client:
INFO 🔍 asking director for worker assignment
INFO connection.id=conn-abc123 worker=worker-b: 🔀 director assigned worker - establishing direct connection
INFO connection.id=conn-abc123 worker=worker-b: ✅ direct connection established to worker

Worker-b:
INFO connection.id=conn-abc123 worker=worker-b: ✅ received inference request from client (direct connection)
INFO connection.id=conn-abc123 worker=worker-b: 🔄 worker confirmed connection
INFO connection.id=conn-abc123 worker=worker-b sequence=1: 📦 received token
INFO connection.id=conn-abc123 worker=worker-b sequence=2: 📦 received token
...
```

Notice the **connection_id remains constant** - that's the logical session maintained across different workers and different QUIC connections!

## 🧪 Testing

All binaries compile and are ready to run:

```bash
# Build everything
cargo build --manifest-path Cargo.toml

# Run individual components
cargo run --bin director
cargo run --bin worker  
cargo run --bin client
```

Or use the automated demo:

```bash
./run_demo.sh
```

## ✅ Current Features

- **Heartbeat-based Failure Detection** - Client detects worker failures via periodic heartbeats (5s interval, 3s timeout)
- **Automatic Reconnection** - Client automatically requests new worker from director on failure
- **Worker Availability Management** - Workers mark themselves unavailable during recovery and reject new connections
- **Randomized Failure Timing** - Workers fail at random intervals (10-25s jitter) to test system resilience
- **Auto-healing System** - All components recover automatically, even when both workers fail simultaneously
- **Round-robin Load Balancing** - Director assigns workers in round-robin fashion
- **Session Continuity** - Connection ID maintained across worker reassignments

## 🔮 Potential Future Enhancements

If this example were to evolve further, here are some possibilities:

1. **Cluster Framework Integration** - Use SWIM gossip protocol for worker discovery (see CLUSTER_DESIGN.md)
2. **Load-aware Routing** - Smart worker assignment based on actual load metrics
3. **Metrics & Observability** - Production-grade monitoring with Prometheus/Grafana
4. **Configurable Failure Simulation** - ENV vars for jitter ranges, recovery times
5. **Multi-director HA** - Director high availability with leader election

The current implementation demonstrates the core resilience patterns effectively!

## 🎓 What You Learn

1. **Streaming RPC Patterns** - Using `register_streaming` and `call_streaming`
2. **Worker Pool Management** - Round-robin assignment and failover
3. **Error Handling** - Graceful degradation and automatic retry
4. **Heartbeat-based Failure Detection** - Client-side health monitoring
5. **Multi-Process Coordination** - Director/worker architecture
6. **Session Continuity** - Maintaining logical sessions across physical connections

## 📝 License

Same as rpcnet.
