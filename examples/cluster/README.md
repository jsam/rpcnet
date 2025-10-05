# Cluster Example

This example demonstrates the **RPC cluster architecture** with automatic worker discovery, load balancing, and failure detection using gossip protocol. It uses **generated RPC code** from `.rpc.rs` trait definitions via `rpcnet-gen`.

## Architecture

### Components

1. **Director** - Coordinator node that:
   - Uses `WorkerRegistry` for automatic worker discovery
   - Uses `ClusterClient` for load-balanced request routing
   - Employs `LeastConnections` strategy
   - Monitors worker pool status
   - Sends periodic requests to workers

2. **Worker** - Processing nodes that:
   - Join cluster automatically via gossip
   - Tag themselves with `role=worker` for discovery
   - Process `compute` tasks
   - Monitor cluster events

### vs Connection Swap Example

**Connection Swap (Manual):**
- Custom `WorkerPool` with HashMap
- Manual round-robin selection
- Explicit worker registration via RPC
- Manual health checks

**Cluster (Automatic):**
- Built-in `WorkerRegistry` + `ClusterClient`
- Configurable load balancing (RoundRobin/Random/LeastConnections)
- Automatic discovery via gossip protocol
- Built-in Phi Accrual failure detection

## Running the Example

### Prerequisites
```bash
# Ensure test certificates exist
ls certs/test_cert.pem certs/test_key.pem

# Note: All commands should be run from the project root directory
```

### Basic Setup (No Failures)

**Terminal 1 - Director:**
```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director
```

**Terminal 2 - Worker A:**
```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=true \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 3 - Worker B:**
```bash
WORKER_LABEL=worker-b \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=true \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 4 - Client:**
```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client
```

### Testing Failure Simulation & Client Failover

To observe automatic failover when workers fail, start workers with `WORKER_FAILURE_ENABLED=true`. Workers will cycle through failures every ~18 seconds (10s run + 3s warning + 5s failed).

**Terminal 1 - Director:**
```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director
```

**Terminal 2 - Worker A (with failures):**
```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=true \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 3 - Worker B (with failures):**
```bash
WORKER_LABEL=worker-b \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=true \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 4 - Client:**
```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client
```

**Expected Behavior:**
- `⚠️  [worker-name] Simulating worker failure in 3 seconds...` - Warning before failure
- `💥 [worker-name] Worker failed!` - Worker enters failed state
- `🔄 [worker-name] Worker recovering...` - Worker begins recovery
- `✅ [worker-name] Worker recovered and ready to serve!` - Worker back online
- Client detects failure via error response and returns to director for new worker assignment
- Streaming continues with minimal interruption as client switches between workers

## What Happens

1. **Director starts** and creates empty cluster
2. **Workers join** cluster by connecting to director (seed node)
3. **Gossip spreads** worker information to director
4. **Director discovers workers** via cluster events
5. **WorkerRegistry** tracks available workers
6. **ClusterClient** routes requests using LeastConnections strategy
7. **Workers process** tasks and return responses
8. **Failure detection** automatically removes failed workers

## Additional Testing Scenarios

### Hard Kill Test (Network-Level Failure)

Kill a worker completely to test network-level failure detection:

```bash
# Press Ctrl+C on worker terminal
```

**Observe:**
- Director detects failure via gossip protocol
- WorkerRegistry removes worker from pool
- Client requests route to remaining healthy workers
- Zero downtime for clients

### Worker Restart Test

Restart a killed worker to see automatic re-discovery:

```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=false \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Observe:**
- Worker rejoins cluster automatically
- Gossip spreads worker availability
- Director adds worker back to registry
- Client requests resume to all available workers

## Expected Output

### Director
```
🎯 Starting Director at 127.0.0.1:61000
📁 Loading certificates from "../../certs/test_cert.pem" and "../../certs/test_key.pem"
✅ Director registered itself in cluster
✅ Cluster enabled - Director is now discoverable
🔄 Load balancing strategy: LeastConnections
📊 Worker pool status: 2 workers available
   - worker-a at 127.0.0.1:62001 (0 connections)
   - worker-b at 127.0.0.1:62002 (0 connections)
🚀 Director ready - listening on 127.0.0.1:61000
📨 client requesting worker assignment
✅ assigned worker to client
```

### Worker (Normal Operation)
```
👷 Starting Worker 'worker-a' at 127.0.0.1:62001
📁 Loading certificates from "../../certs/test_cert.pem" and "../../certs/test_key.pem"
🔌 Binding server to 127.0.0.1:62001...
✅ Server bound successfully to 127.0.0.1:62001
🌐 Enabling cluster, connecting to director at 127.0.0.1:61000...
✅ Cluster enabled, connected to director
🏷️  Tagging worker with role=worker and label=worker-a...
✅ Worker 'worker-a' joined cluster with role=worker, label=worker-a
🚀 Worker 'worker-a' is running and ready to handle requests
🎬 [worker-a] Streaming handler invoked
✅ received inference request from client (direct connection)
```

### Worker (With Failure Simulation)
```
👷 Starting Worker 'worker-a' at 127.0.0.1:62001
[... same startup as above ...]
⚠️  [worker-a] Simulating worker failure in 3 seconds...
💥 [worker-a] Worker failed!
🔄 [worker-a] Worker recovering...
✅ [worker-a] Worker recovered and ready to serve!
[cycle repeats every ~18 seconds]
```

### Client
```
📡 Starting Client - connecting to director at 127.0.0.1:61000
🔍 asking director for worker assignment
✅ connected to director
🔀 director assigned worker - establishing direct connection
✅ direct connection established to worker
📤 creating request stream
🔌 calling generate on worker
🌊 stream opened successfully, starting to consume responses
🔄 worker confirmed connection
📦 received token (sequence=1, text="token-1", total=1)
📦 received token (sequence=2, text="token-2", total=2)
[... continues receiving tokens ...]
⚠️  worker failed - will request new worker from director
🔄 returning to director for new worker assignment
[... repeats cycle with new worker ...]
```

## Key Cluster Features Demonstrated

### 1. Automatic Discovery
- No manual registration needed
- Workers auto-discovered via gossip
- Director learns about workers through cluster events

### 2. Load Balancing
- `LeastConnections` strategy balances load
- Tracks active connections per worker
- Prevents overload on any single worker

### 3. Failure Detection
- Phi Accrual failure detector
- Gossip protocol spreads failure information
- Automatic worker removal

### 4. Tag-Based Routing
- Workers tagged with `role=worker`
- Director filters by role when selecting
- Enables heterogeneous worker pools (GPU, CPU, etc.)

### 5. Event Monitoring
- Workers subscribe to cluster events
- Real-time visibility into cluster changes
- NodeJoined, NodeLeft, NodeFailed events

## Code Comparison

### Manual (Connection Swap)
```rust
// ~200 lines of custom code
WorkerPool {
    workers: HashMap<Uuid, WorkerInfo>,
    next_worker_idx: usize,
}

// Manual registration
async fn register(&self, worker: WorkerInfo) -> Uuid { ... }

// Manual selection
async fn get_next_worker(&self) -> Option<WorkerInfo> { ... }

// Manual health checks
async fn check_worker_health(&self, worker_id: Uuid) -> bool { ... }
```

### Cluster (This Example)
```rust
// ~50 lines using cluster APIs
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;

let cluster_client = Arc::new(ClusterClient::new(registry, config));

// Automatic discovery, selection, and health checks!
let result = cluster_client.call_worker("compute", data, filter).await?;
```

**Net reduction:** ~150 lines of code removed!

## Benefits Over Manual Approach

1. **Less Code** - 75% reduction in worker management code
2. **More Robust** - Gossip handles network partitions
3. **Better Health Checks** - Phi Accrual vs simple ping
4. **Automatic Discovery** - No explicit registration
5. **Flexible Strategies** - Easy to switch load balancing
6. **Tag-Based Routing** - Filter by capabilities
7. **Production Ready** - Battle-tested SWIM protocol

## Configuration Options

### Environment Variables

#### Director
- `DIRECTOR_ADDR` - Director bind address (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

#### Worker
- `WORKER_LABEL` - Worker identifier (default: `worker-1`)
- `WORKER_ADDR` - Worker bind address (default: `127.0.0.1:62001`)
- `DIRECTOR_ADDR` - Director address to connect to (default: `127.0.0.1:61000`)
- `WORKER_FAILURE_ENABLED` - Enable periodic complete worker failures (default: `false`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

#### Client
- `DIRECTOR_ADDR` - Director address to connect to (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

### Load Balancing Strategies
```rust
LoadBalancingStrategy::RoundRobin       // Even distribution
LoadBalancingStrategy::Random           // Random selection
LoadBalancingStrategy::LeastConnections // Pick least loaded
```

### Cluster Config
```rust
ClusterConfig::default()
    .with_gossip_interval(Duration::from_secs(1))
    .with_health_check_interval(Duration::from_secs(2))
```

## Troubleshooting

**Workers not discovered:**
- Ensure director starts first (seed node)
- Check firewall allows UDP gossip
- Verify workers connect to correct director address

**Requests failing:**
- Check worker has `role=worker` tag
- Verify compute handler is registered
- Check logs for connection errors

## Next Steps

- Try different load balancing strategies
- Add more workers dynamically
- Test network partition scenarios
- Monitor cluster events
- Add custom tags for routing (zone, gpu, etc.)
