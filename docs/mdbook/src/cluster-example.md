# Cluster Example

This chapter demonstrates building a distributed RPC cluster with automatic worker discovery, load balancing, and failure detection using RpcNet's built-in cluster features.

## Architecture Overview

The cluster example showcases three main components working together:

```
                    ┌──────────────────────────┐
                    │      Director            │
                    │  (Coordinator Node)      │
                    │                          │
                    │  - WorkerRegistry        │
                    │  - ClusterClient         │
                    │  - Load Balancing        │
                    └────────┬─────────────────┘
                             │
                    Gossip Protocol (SWIM)
                             │
            ┌────────────────┼────────────────┐
            │                                 │
    ┌───────▼────────┐              ┌────────▼───────┐
    │   Worker A      │              │   Worker B      │
    │                 │              │                 │
    │  - Auto-join    │              │  - Auto-join    │
    │  - Tag: worker  │              │  - Tag: worker  │
    │  - Process tasks│              │  - Process tasks│
    └─────────────────┘              └─────────────────┘
```

### Components

**1. Director** - Coordinator node that:
- Uses `WorkerRegistry` for automatic worker discovery
- Uses `ClusterClient` for load-balanced request routing
- Employs `LeastConnections` strategy by default
- Monitors worker pool status
- Routes client requests to healthy workers

**2. Workers** - Processing nodes that:
- Join cluster automatically via gossip protocol
- Tag themselves with `role=worker` for discovery
- Process compute tasks from clients
- Monitor cluster events (node joined/left/failed)
- Support simulated failures for testing

**3. Client** - Application that:
- Connects to director
- Gets worker assignment
- Establishes direct connection to worker
- Handles failover automatically

## Why Use Built-in Cluster Features?

Compared to manual worker management patterns:

**Manual Approach** ❌:
- Custom `HashMap<Uuid, WorkerInfo>` for tracking
- Manual round-robin selection logic
- Explicit RPC calls for worker registration
- Custom ping-based health checks
- ~200 lines of boilerplate code

**Built-in Cluster** ✅:
- Built-in `WorkerRegistry` + `ClusterClient`
- Multiple load balancing strategies (Round Robin, Random, Least Connections)
- Automatic discovery via SWIM gossip protocol
- Phi Accrual failure detection (accurate, adaptive)
- ~50 lines to set up
- **75% code reduction!**

## Running the Example

### Prerequisites

Ensure test certificates exist:
```bash
ls certs/test_cert.pem certs/test_key.pem
```

All commands should be run from the **project root directory**.

### Basic Setup

Open four terminals and run each component:

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
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 3 - Worker B:**
```bash
WORKER_LABEL=worker-b \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 4 - Client:**
```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client
```

### What You'll See

**Director Output:**
```
🎯 Starting Director at 127.0.0.1:61000
📁 Loading certificates from "../../certs/test_cert.pem"
✅ Director registered itself in cluster
✅ Cluster enabled - Director is now discoverable
🔄 Load balancing strategy: LeastConnections
📊 Worker pool status: 2 workers available
   - worker-a at 127.0.0.1:62001 (0 connections)
   - worker-b at 127.0.0.1:62002 (0 connections)
🚀 Director ready - listening on 127.0.0.1:61000
```

**Worker Output:**
```
👷 Starting Worker 'worker-a' at 127.0.0.1:62001
🔌 Binding server to 127.0.0.1:62001...
✅ Server bound successfully
🌐 Enabling cluster, connecting to director at 127.0.0.1:61000...
✅ Cluster enabled, connected to director
🏷️  Tagging worker with role=worker and label=worker-a...
✅ Worker 'worker-a' joined cluster with role=worker
🚀 Worker 'worker-a' is running and ready to handle requests
```

**Client Output:**
```
📡 Starting Client - connecting to director at 127.0.0.1:61000
✅ connected to director
🔀 director assigned worker - establishing direct connection
✅ direct connection established to worker
📤 creating request stream
🌊 stream opened successfully, starting to consume responses
📦 received token (sequence=1, text="token-1", total=1)
📦 received token (sequence=2, text="token-2", total=2)
...
```

## Testing Failure Scenarios

### Simulated Worker Failures

Enable periodic failures to test automatic failover:

**Worker with Failures:**
```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  WORKER_FAILURE_ENABLED=true \  # Enable failure simulation
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Failure Cycle** (~18 seconds):
1. **Run**: 10 seconds of normal operation
2. **Warning**: "⚠️  Simulating worker failure in 3 seconds..."
3. **Failed**: 5 seconds in failed state - "💥 Worker failed!"
4. **Recovery**: "🔄 Worker recovering..."
5. **Ready**: "✅ Worker recovered and ready to serve!"
6. Repeat

**Client Behavior:**
- Detects failure via error response
- Returns to director for new worker assignment
- Switches to healthy worker seamlessly
- Streaming continues with minimal interruption

### Hard Kill Test

Test network-level failure detection:

```bash
# In a worker terminal, press Ctrl+C
```

**Observe:**
- Director detects failure via gossip protocol
- `WorkerRegistry` removes worker from pool
- Client requests automatically route to remaining workers
- Zero downtime for ongoing operations

### Worker Restart Test

After killing a worker, restart it to see re-discovery:

```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Observe:**
- Worker automatically rejoins cluster
- Gossip spreads worker availability
- Director adds worker back to registry
- Client requests resume to all available workers

## How It Works

### 1. Automatic Discovery

Workers don't manually register - they just join the cluster:

```rust
// Worker code (simplified)
let cluster = ClusterMembership::new(config).await?;
cluster.join(vec![director_addr]).await?;

// Tag for discovery
cluster.set_tag("role", "worker");
cluster.set_tag("label", worker_label);

// That's it! Director discovers automatically via gossip
```

### 2. Load Balancing

Director uses `WorkerRegistry` for automatic load balancing:

```rust
// Director code
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;

// Automatically tracks workers and balances load
```

### 3. Failure Detection

Phi Accrual algorithm provides accurate health monitoring:

- Adapts to network conditions
- Distinguishes slow nodes from failed nodes
- No false positives from temporary delays
- Automatic recovery when nodes return

### 4. Tag-Based Routing

Filter workers by capabilities:

```rust
// Get only GPU workers
let gpu_worker = registry.select_worker(Some("gpu=true")).await?;

// Get any worker
let any_worker = registry.select_worker(Some("role=worker")).await?;
```

## Key Cluster Features Demonstrated

### ✅ Automatic Discovery
No manual registration needed - gossip protocol handles everything

### ✅ Load Balancing
Choose from:
- **Round Robin**: Even distribution
- **Random**: Stateless workload distribution
- **Least Connections**: Balance based on current load (recommended)

### ✅ Failure Detection
Phi Accrual algorithm provides accurate, adaptive health monitoring

### ✅ Connection Pooling
Efficient connection reuse with configurable settings

### ✅ Tag-Based Routing
Route by worker capabilities (GPU, CPU, zone, etc.)

### ✅ Event Monitoring
Subscribe to cluster events:
- `NodeJoined` - New worker available
- `NodeLeft` - Worker gracefully departed
- `NodeFailed` - Worker detected as failed

## Configuration Options

### Environment Variables

**Director:**
- `DIRECTOR_ADDR` - Bind address (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

**Worker:**
- `WORKER_LABEL` - Worker identifier (default: `worker-1`)
- `WORKER_ADDR` - Bind address (default: `127.0.0.1:62001`)
- `DIRECTOR_ADDR` - Director address (default: `127.0.0.1:61000`)
- `WORKER_FAILURE_ENABLED` - Enable failure simulation (default: `false`)
- `RUST_LOG` - Log level

**Client:**
- `DIRECTOR_ADDR` - Director address (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level

### Load Balancing Strategies

```rust
use rpcnet::cluster::LoadBalancingStrategy;

// Options:
LoadBalancingStrategy::RoundRobin       // Even distribution
LoadBalancingStrategy::Random           // Random selection
LoadBalancingStrategy::LeastConnections // Pick least loaded (recommended)
```

### Cluster Configuration

```rust
use rpcnet::cluster::ClusterConfig;

let config = ClusterConfig::default()
    .with_gossip_interval(Duration::from_secs(1))
    .with_health_check_interval(Duration::from_secs(2));
```

## Troubleshooting

**Workers not discovered:**
- Ensure director starts first (it's the seed node)
- Check firewall allows UDP for gossip
- Verify workers connect to correct director address

**Requests failing:**
- Check worker has `role=worker` tag
- Verify compute handler is registered
- Check logs for connection errors

**Slow failover:**
- Adjust health check interval in config
- Tune Phi Accrual threshold
- Check network latency

## Production Considerations

For production deployments:

1. **TLS Certificates**: Use proper certificates, not test certs
2. **Monitoring**: Integrate cluster events with your monitoring system
3. **Scaling**: Add more workers dynamically as needed
4. **Persistence**: Consider persisting cluster state if needed
5. **Security**: Add authentication and authorization
6. **Network**: Plan for network partitions and split-brain scenarios

## Next Steps

- Try different load balancing strategies
- Add more workers dynamically
- Test network partition scenarios
- Add custom tags for routing (zone, GPU, etc.)
- Integrate with your application logic

For full source code, see `examples/cluster/` in the repository.
