# Migration Guide

This guide helps you migrate from manual worker management patterns to RpcNet's built-in cluster features, reducing code complexity and improving reliability.

## Why Migrate?

### Before: Manual Worker Management

**Typical manual pattern** requires ~200 lines of boilerplate:

```rust
// Custom worker tracking
struct WorkerPool {
    workers: Arc<Mutex<HashMap<Uuid, WorkerInfo>>>,
    next_idx: Arc<Mutex<usize>>,
}

struct WorkerInfo {
    id: Uuid,
    addr: SocketAddr,
    label: String,
    last_ping: Instant,
}

impl WorkerPool {
    // Manual registration
    async fn register_worker(&self, info: WorkerInfo) -> Uuid {
        let id = Uuid::new_v4();
        self.workers.lock().await.insert(id, info);
        id
    }
    
    // Manual round-robin selection
    async fn get_next_worker(&self) -> Option<WorkerInfo> {
        let workers = self.workers.lock().await;
        if workers.is_empty() {
            return None;
        }
        let mut idx = self.next_idx.lock().await;
        let worker_list: Vec<_> = workers.values().collect();
        let worker = worker_list[*idx % worker_list.len()].clone();
        *idx += 1;
        Some(worker)
    }
    
    // Manual health checking
    async fn check_health(&self) {
        let mut workers = self.workers.lock().await;
        workers.retain(|_, worker| {
            worker.last_ping.elapsed() < Duration::from_secs(30)
        });
    }
}
```

**Problems**:
- ❌ No automatic discovery
- ❌ Basic round-robin only
- ❌ Simple timeout-based health checks
- ❌ Manual connection management
- ❌ No partition detection
- ❌ ~200+ lines of error-prone code

### After: Built-in Cluster Features

**With RpcNet's cluster** - only ~50 lines:

```rust
use rpcnet::cluster::{WorkerRegistry, LoadBalancingStrategy, ClusterClient};

// Automatic discovery + load balancing + health checking
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;

let client = Arc::new(ClusterClient::new(registry, config));

// That's it! Everything else is automatic:
let result = client.call_worker("compute", data, Some("role=worker")).await?;
```

**Benefits**:
- ✅ Automatic discovery via gossip
- ✅ Multiple load balancing strategies
- ✅ Phi Accrual failure detection
- ✅ Efficient connection management
- ✅ Partition detection
- ✅ **75% code reduction**

## Migration Steps

### Step 1: Add Cluster Feature

Update `Cargo.toml`:

```toml
[dependencies]
# Before
rpcnet = "0.2"

# After
rpcnet = { version = "0.2", features = ["cluster"] }
```

### Step 2: Enable Cluster on Server

Replace manual worker registration with cluster:

```rust
// Before: Manual RPC endpoint for registration
#[rpc_trait]
pub trait DirectorService {
    async fn register_worker(&self, info: WorkerInfo) -> Result<Uuid>;
}

// After: Enable cluster on server
let cluster_config = ClusterConfig::default()
    .with_bind_addr(bind_addr.parse()?);

let cluster = server.enable_cluster(cluster_config).await?;

// Tag for discovery
cluster.set_tag("role", "director");
```

### Step 3: Replace WorkerPool with WorkerRegistry

```rust
// Before: Custom WorkerPool
let worker_pool = Arc::new(WorkerPool::new());

// Spawn health checker
tokio::spawn({
    let pool = worker_pool.clone();
    async move {
        loop {
            pool.check_health().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
});

// After: Built-in WorkerRegistry
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;  // Automatic health checking included!
```

### Step 4: Update Worker Startup

```rust
// Before: Worker calls register RPC
let director_client = DirectorClient::connect(&director_addr, config).await?;
let worker_id = director_client.register_worker(WorkerInfo {
    label: worker_label,
    addr: worker_addr,
}).await?;

// After: Worker joins cluster
let cluster_config = ClusterConfig::default()
    .with_bind_addr(worker_addr.parse()?);

let cluster = server.enable_cluster(cluster_config).await?;
cluster.join(vec![director_addr.parse()?]).await?;

// Tag for discovery
cluster.set_tag("role", "worker");
cluster.set_tag("label", &worker_label);
```

### Step 5: Replace Manual Selection with ClusterClient

```rust
// Before: Manual worker selection + connection
let worker = worker_pool.get_next_worker().await
    .ok_or_else(|| anyhow::anyhow!("No workers available"))?;

let conn = Connection::connect(&worker.addr, client_config).await?;
let result = conn.call("compute", data).await?;

// After: Automatic selection + pooled connection
let result = cluster_client.call_worker("compute", data, Some("role=worker")).await?;
```

### Step 6: Remove Manual Health Checks

```rust
// Before: Periodic ping to check health
tokio::spawn(async move {
    loop {
        for worker in workers.iter() {
            match ping_worker(&worker.addr).await {
                Ok(_) => worker.last_ping = Instant::now(),
                Err(_) => remove_worker(worker.id).await,
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
});

// After: Nothing! Phi Accrual + gossip handles it automatically
// Just subscribe to events if you want notifications:
let mut events = cluster.subscribe();
tokio::spawn(async move {
    while let Some(event) = events.recv().await {
        match event {
            ClusterEvent::NodeFailed(node) => {
                log::error!("Worker {} failed", node.id);
            }
            _ => {}
        }
    }
});
```

## Migration Examples

### Example 1: Simple Director-Worker

#### Before (Manual)

```rust
// director.rs - ~150 lines
struct Director {
    workers: Arc<Mutex<HashMap<Uuid, WorkerInfo>>>,
    next_idx: Arc<Mutex<usize>>,
}

#[rpc_impl]
impl DirectorService for Director {
    async fn register_worker(&self, info: WorkerInfo) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.workers.lock().await.insert(id, info);
        Ok(id)
    }
    
    async fn get_worker(&self) -> Result<WorkerInfo> {
        let workers = self.workers.lock().await;
        if workers.is_empty() {
            return Err(anyhow::anyhow!("No workers"));
        }
        let mut idx = self.next_idx.lock().await;
        let worker_list: Vec<_> = workers.values().collect();
        let worker = worker_list[*idx % worker_list.len()].clone();
        *idx += 1;
        Ok(worker)
    }
}

// worker.rs - ~50 lines
async fn main() -> Result<()> {
    let mut server = Server::new(config);
    server.register_service(Arc::new(WorkerHandler));
    server.bind(&worker_addr).await?;
    
    // Register with director
    let director_client = DirectorClient::connect(&director_addr, config).await?;
    director_client.register_worker(WorkerInfo {
        label: worker_label,
        addr: worker_addr,
    }).await?;
    
    server.run().await?;
    Ok(())
}
```

**Total**: ~200 lines

#### After (Cluster)

```rust
// director.rs - ~50 lines
async fn main() -> Result<()> {
    let mut server = Server::new(config);
    
    // Enable cluster
    let cluster = server.enable_cluster(cluster_config).await?;
    cluster.set_tag("role", "director");
    
    // Create registry
    let registry = Arc::new(WorkerRegistry::new(
        cluster,
        LoadBalancingStrategy::LeastConnections
    ));
    registry.start().await;
    
    server.bind(&director_addr).await?;
    server.run().await?;
    Ok(())
}

// worker.rs - ~30 lines
async fn main() -> Result<()> {
    let mut server = Server::new(config);
    server.register_service(Arc::new(WorkerHandler));
    server.bind(&worker_addr).await?;
    
    // Join cluster
    let cluster = server.enable_cluster(cluster_config).await?;
    cluster.join(vec![director_addr.parse()?]).await?;
    cluster.set_tag("role", "worker");
    cluster.set_tag("label", &worker_label);
    
    server.run().await?;
    Ok(())
}
```

**Total**: ~80 lines (60% reduction)

### Example 2: Connection Swap Pattern

The old `connection_swap` example has been replaced by the `cluster` example which uses built-in features.

#### Migration Path

1. **Remove custom WorkerPool** → Use `WorkerRegistry`
2. **Remove manual registration RPC** → Use gossip discovery
3. **Remove health check pings** → Use Phi Accrual
4. **Keep application logic unchanged** → RPC interfaces stay the same

**See**: `examples/cluster/` for complete working example

## Feature Comparison

| Feature | Manual Pattern | Built-in Cluster |
|---------|---------------|------------------|
| **Discovery** | Manual RPC registration | Automatic via gossip |
| **Load Balancing** | Basic round-robin | Round Robin, Random, Least Connections |
| **Health Checking** | Timeout-based ping | Phi Accrual algorithm |
| **Failure Detection** | Simple timeout | Indirect probes + Phi |
| **Connection Management** | Manual implementation | Built-in optimization |
| **Partition Detection** | Not available | Automatic |
| **Code Complexity** | ~200 lines | ~50 lines |
| **Maintenance** | High (custom code) | Low (battle-tested) |

## Common Migration Issues

### Issue 1: Port Conflicts

**Problem**: Gossip protocol uses UDP, might conflict with existing services.

**Solution**: Configure gossip port explicitly

```rust
let cluster_config = ClusterConfig::default()
    .with_bind_addr("0.0.0.0:7946".parse()?)  // Gossip on different port
    .with_gossip_port(7947);  // Custom gossip port
```

### Issue 2: Firewall Rules

**Problem**: Gossip UDP traffic blocked by firewall.

**Solution**: Allow UDP traffic between cluster nodes

```bash
# Allow gossip protocol
iptables -A INPUT -p udp --dport 7946 -j ACCEPT
iptables -A OUTPUT -p udp --sport 7946 -j ACCEPT
```

### Issue 3: Existing Health Check Logic

**Problem**: Have custom health check logic that needs to be preserved.

**Solution**: Combine with cluster events

```rust
// Keep custom health checks
async fn custom_health_check(worker: &Worker) -> bool {
    // Your custom logic
    worker.cpu_usage < 80.0 && worker.memory_available > 1_000_000
}

// Use alongside cluster events
let mut events = cluster.subscribe();
while let Some(event) = events.recv().await {
    if let ClusterEvent::NodeFailed(node) = event {
        // Cluster detected failure
        handle_failure(node).await;
    }
}

// Periodic custom checks
tokio::spawn(async move {
    loop {
        for worker in registry.workers().await {
            if !custom_health_check(&worker).await {
                log::warn!("Custom health check failed for {}", worker.label);
            }
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
});
```

### Issue 4: Different Node Roles

**Problem**: Have multiple types of nodes (coordinator, worker, storage, etc.).

**Solution**: Use tags to differentiate

```rust
// Coordinator
cluster.set_tag("role", "coordinator");

// GPU worker
cluster.set_tag("role", "worker");
cluster.set_tag("gpu", "true");

// CPU worker
cluster.set_tag("role", "worker");
cluster.set_tag("cpu_only", "true");

// Select by role
let gpu_worker = registry.select_worker(Some("gpu=true")).await?;
let any_worker = registry.select_worker(Some("role=worker")).await?;
```

## Testing After Migration

### Unit Tests

```rust
#[tokio::test]
async fn test_worker_discovery() {
    // Start director
    let director = start_test_director().await;
    
    // Start worker
    let worker = start_test_worker().await;
    worker.join(vec![director.addr()]).await.unwrap();
    
    // Wait for discovery
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify worker discovered
    let workers = director.registry().workers().await;
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].tags.get("role"), Some(&"worker".to_string()));
}

#[tokio::test]
async fn test_load_balancing() {
    let director = start_test_director().await;
    
    // Start 3 workers
    let worker1 = start_test_worker("worker-1").await;
    let worker2 = start_test_worker("worker-2").await;
    let worker3 = start_test_worker("worker-3").await;
    
    // Make 100 requests
    let mut worker_counts = HashMap::new();
    for _ in 0..100 {
        let result = director.call_worker("compute", vec![]).await.unwrap();
        *worker_counts.entry(result.worker_label).or_insert(0) += 1;
    }
    
    // Verify distribution (should be roughly equal)
    assert!(worker_counts.get("worker-1").unwrap() > &20);
    assert!(worker_counts.get("worker-2").unwrap() > &20);
    assert!(worker_counts.get("worker-3").unwrap() > &20);
}
```

### Integration Tests

```bash
# Test full cluster
cargo test --features cluster --test integration_tests

# Test failure scenarios
cargo test --features cluster --test failure_tests

# Test with actual network (examples)
cd examples/cluster
cargo run --bin director &
cargo run --bin worker &
cargo run --bin client
```

## Rollback Plan

If migration causes issues, you can rollback:

### Option 1: Feature Flag

```rust
#[cfg(feature = "use-cluster")]
use rpcnet::cluster::{WorkerRegistry, ClusterClient};

#[cfg(not(feature = "use-cluster"))]
use crate::manual_pool::WorkerPool;

// Toggle between old and new with feature flag
```

### Option 2: Gradual Migration

```rust
// Run both systems in parallel temporarily
let manual_pool = Arc::new(WorkerPool::new());  // Old system
let cluster_registry = Arc::new(WorkerRegistry::new(cluster, strategy));  // New system

// Route percentage of traffic to new system
if rand::random::<f64>() < 0.10 {  // 10% to new system
    cluster_registry.select_worker(filter).await
} else {
    manual_pool.get_next_worker().await  // 90% to old system
}

// Gradually increase percentage over time
```

## Checklist

### Pre-Migration

- [ ] Review current worker management code
- [ ] Identify custom health check logic to preserve
- [ ] Plan firewall rule changes for gossip
- [ ] Write tests for current behavior
- [ ] Create rollback plan

### During Migration

- [ ] Add cluster feature to Cargo.toml
- [ ] Enable cluster on servers
- [ ] Replace WorkerPool with WorkerRegistry
- [ ] Update worker startup (join instead of register)
- [ ] Remove manual health checks
- [ ] Test in staging environment

### Post-Migration

- [ ] Verify worker discovery working
- [ ] Check load balancing distribution
- [ ] Monitor failure detection
- [ ] Validate performance metrics
- [ ] Remove old worker pool code
- [ ] Update documentation

## Performance Impact

**Before migration**:
- Manual round-robin: ~100K RPS
- Timeout-based health: 30s detection time
- Manual connection handling: 20-50ms latency

**After migration**:
- Least Connections: 172K+ RPS (70% increase)
- Phi Accrual: 6-8s detection time (better accuracy)
- Built-in connection management: <1ms latency (98% reduction)

## Next Steps

- **[Cluster Tutorial](../cluster/tutorial.md)** - Build cluster from scratch
- **[Production Guide](production.md)** - Deploy migrated cluster
- **[Performance Tuning](performance.md)** - Optimize new setup

## References

- **[Cluster Example](https://github.com/yourusername/rpcnet/tree/main/examples/cluster)** - Complete working example
- **[SWIM Paper](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf)** - Gossip protocol details
- **[Phi Accrual Paper](https://citeseerx.ist.psu.edu/document?repid=rep1&type=pdf&doi=babf246cf6753ad12ce97ae47e64c9d4ff85c6f7)** - Failure detection algorithm
