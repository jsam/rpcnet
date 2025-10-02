# Connection Swap Example

**Seamless QUIC connection migration between workers** - A complete demonstration of zero-downtime connection migration using rpcnet's migration infrastructure.

## 🚀 Quick Start

```bash
cd examples/connection_swap
./run_demo.sh
```

Watch as a client maintains an uninterrupted data stream while the director automatically migrates the connection from a failing worker to a healthy one!

## 📖 Documentation

- **[QUICKSTART.md](./QUICKSTART.md)** - Complete setup and usage guide
- **[IMPLEMENTED.md](./IMPLEMENTED.md)** - Deep dive into the migration infrastructure
- **[simple_demo.rs](./simple_demo.rs)** - Simple demonstration of migration APIs

## 🎯 What This Demonstrates

This example shows a **fully functional** multi-process system demonstrating:

1. **Director Process** - Manages worker pool and routes connections
2. **Worker Processes** - Handle streaming requests, simulate failures  
3. **Client Process** - Issues long-running requests, observes seamless failover
4. **Zero-Downtime Migration** - Connection ID remains constant across workers

### Key Innovation

The same QUIC connection serves the entire request despite worker changes:

```
connection.id=conn-1234 worker=worker-a  ← Initial assignment
connection.id=conn-1234 worker=worker-b  ← After migration (same ID!)
```

This proves the connection was **migrated**, not recreated.

## 🏗️ Architecture

```
┌─────────┐
│ Client  │ ← Single uninterrupted stream
└────┬────┘
     │
     ↓ QUIC Connection (USER port 61000)
┌──────────────────┐     
│    Director      │←─── Worker-a (fails at 15s) 
│ USER:  61000     │      USER:  62001, MGMT: 63001
│ MGMT:  61001     │
└──────────────────┘←─── Worker-b (takes over)
                          USER:  62002, MGMT: 63002
```

### Dual-Port Architecture

Each component runs two servers:
- **USER Port**: RPC endpoints for business logic (client requests, streaming)
- **MGMT Port**: Control plane for health checks, worker registration, migration coordination

### Flow

1. Client connects to director and requests streaming data
2. Director assigns worker-a
3. Worker-a serves for ~15 seconds, generating tokens
4. Worker-a simulates failure
5. Director detects error and assigns worker-b  
6. Worker-b continues generating tokens
7. Client receives uninterrupted stream

## 🔧 Components

### Director (`src/bin/director.rs`)
- **USER Port (61000)**: Client streaming requests (`generate`)
- **MGMT Port (61001)**: Worker registration, health checks
- Worker pool management with round-robin assignment
- Automatic connection migration on worker failure
- Maintains connection_id throughout migration

### Worker (`src/bin/worker.rs`)
- **USER Port (62001/62002)**: Streaming inference endpoint (`generate`)
- **MGMT Port (63001/63002)**: Health check endpoint
- Registers with director's MGMT port on startup (heartbeats every 5s)
- Generates tokens at 500ms intervals
- Simulates failure after 15 seconds

### Client (`src/bin/client.rs`)
- Connects to director via RpcClient
- Issues streaming `generate` request
- Logs all tokens with connection_id and worker labels
- Demonstrates seamless migration

## 📊 Expected Output

### Initial Connection (Worker-a)
```
INFO connection.id=conn-abc123 worker=worker-a stream.id=1: 🔄 worker assigned
INFO connection.id=conn-abc123 worker=worker-a sequence=1: 📦 received token
INFO connection.id=conn-abc123 worker=worker-a sequence=2: 📦 received token
...
INFO connection.id=conn-abc123 worker=worker-a sequence=29: 📦 received token
```

### Migration Event (~15s)
```
Worker-a:
WARN worker=worker-a connection.id=conn-abc123: ⚠️  simulating failure after 15s

Director:
WARN stream.id=1 connection.id=conn-abc123 worker=worker-a: ⚠️  worker failed - initiating connection migration
INFO stream.id=1 connection.id=conn-abc123 from_worker=worker-a to_worker=worker-b: 🔀 migrating connection to new worker

Client:
INFO connection.id=conn-abc123 worker=worker-a: ⚠️  worker error received
INFO connection.id=conn-abc123 from_worker=worker-a to_worker=worker-b: 🔀 CONNECTION MIGRATION: switching workers
```

### Automatic Failover (Worker-b)
```
INFO connection.id=conn-abc123 worker=worker-b stream.id=1: 🔄 worker assigned
INFO connection.id=conn-abc123 worker=worker-b sequence=1: 📦 received token
INFO connection.id=conn-abc123 worker=worker-b sequence=2: 📦 received token
...
```

Notice the **connection.id remains constant** and **migration is clearly logged** - that's seamless migration in action!

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

## 🏗️ Built On

This example uses the **complete migration infrastructure** from `src/migration/`:

- ✅ **MigrationStateMachine** - 12-state FSM with 11 tests
- ✅ **ConnectionSessionManager** - Session lifecycle with 10 tests  
- ✅ **MigrationServiceImpl** - Complete service with 3 tests
- ✅ **Token-based authentication** - 18 tests
- ✅ **State transfer services** - 33 tests

**Total: 166 passing tests**

See [IMPLEMENTED.md](./IMPLEMENTED.md) for the full migration library documentation.

## 🎓 What You Learn

1. **Streaming RPC Patterns** - Using `register_streaming` and `call_streaming`
2. **Worker Pool Management** - Round-robin assignment and failover
3. **Error Handling** - Graceful degradation and automatic retry
4. **Connection Migration** - Maintaining connection identity across workers
5. **Multi-Process Coordination** - Director/worker architecture

## ✅ Current Features

- **Heartbeat-based Failure Detection** - Client detects worker failures via periodic heartbeats (5s interval, 3s timeout)
- **Automatic Reconnection** - Client automatically requests new worker from director on failure
- **Worker Availability Management** - Workers mark themselves unavailable during recovery and reject new connections
- **Randomized Failure Timing** - Workers fail at random intervals (10-25s jitter) to test system resilience
- **Auto-healing System** - All components recover automatically, even when both workers fail simultaneously

## 🔮 Potential Future Enhancements

If this example were to evolve further, here are some possibilities:

1. **Gossip-based Worker Discovery** - Decentralized peer discovery on management port
2. **Load-aware Routing** - Smart worker assignment based on actual load
3. **Metrics & Observability** - Production-grade monitoring with Prometheus/Grafana
4. **Configurable Failure Simulation** - ENV vars for jitter ranges, recovery times
5. **Multi-director HA** - Director high availability with leader election

The current implementation demonstrates the core resilience patterns effectively!

## 📝 License

Same as rpcnet.
