# Failure Handling

Distributed systems must gracefully handle node failures, network partitions, and other failure scenarios. This chapter explains how RpcNet detects and recovers from failures in cluster deployments.

## Types of Failures

### 1. Node Crashes

**Scenario**: Worker process terminates unexpectedly

```
Before:                  After:
  [Director]               [Director]
      |                        |
  ┌───┴───┐               ┌────┴────┐
  A   B   C               A       C
          X ← Crashed
```

**Detection**:
- Gossip protocol detects missing heartbeats
- Phi Accrual marks node as failed (typically 4-8 seconds)
- Failure event propagated to all nodes

**Recovery**:
```rust
// Automatic handling via WorkerRegistry
let mut events = registry.subscribe();

while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeFailed(node) => {
            log::error!("Worker {} failed", node.id);
            // WorkerRegistry automatically removes from pool
            // Future requests route to remaining workers
        }
        _ => {}
    }
}
```

### 2. Network Partitions

**Scenario**: Network split divides cluster

```
Before partition:         After partition:
     Director                Director  |  
      /    \                   /       |     
     A      B                 A        |  B
     
Cluster view splits into two independent groups
```

**Detection**:
- Nodes on each side detect "failures" of nodes on other side
- Partition detector identifies split-brain scenario
- Both sides continue operating independently

**Handling**:
```rust
// Monitor for partitions
let mut events = cluster.subscribe();

while let Some(event) = events.recv().await {
    if let ClusterEvent::PartitionDetected(minority, majority) = event {
        log::error!("Network partition detected!");
        
        if minority.contains(&my_node_id) {
            // I'm in minority partition
            log::warn!("In minority partition, entering degraded mode");
            enter_read_only_mode().await;
        } else {
            // I'm in majority partition
            log::info!("In majority partition, continuing normal operation");
        }
    }
}
```

### 3. Slow Nodes (Degraded Performance)

**Scenario**: Node responding but very slowly

```
Normal response:    100ms
Degraded response:  5000ms (50x slower)
```

**Detection**:
- Phi Accrual increases suspicion level but may not mark as failed
- Request timeouts at application level
- Load balancer (Least Connections) naturally avoids slow nodes

**Handling**:
```rust
// Set request timeout
let timeout = Duration::from_secs(5);

match tokio::time::timeout(timeout, worker.call("compute", data)).await {
    Ok(Ok(result)) => {
        // Success
    }
    Ok(Err(e)) => {
        log::error!("Worker returned error: {}", e);
        retry_with_different_worker(data).await?;
    }
    Err(_) => {
        log::warn!("Worker timeout, trying another");
        retry_with_different_worker(data).await?;
    }
}
```

### 4. Cascading Failures

**Scenario**: Failure of one node causes others to fail

```
Worker A crashes
  → Remaining workers overloaded
    → Worker B crashes from overload
      → Worker C also crashes
        → Complete system failure
```

**Prevention**:
```rust
// Load shedding to prevent cascading failures
async fn select_worker_with_shedding(
    registry: &WorkerRegistry,
    max_load: f64,
) -> Result<Worker> {
    let worker = registry.select_worker(Some("role=worker")).await?;
    
    let load = worker.active_connections as f64 / worker.capacity as f64;
    
    if load > max_load {
        // Reject request to prevent overload
        return Err(anyhow::anyhow!("All workers at capacity, shedding load"));
    }
    
    Ok(worker)
}
```

## Failure Detection Timeline

### Node Crash Detection

```
Time:    0s      1s      2s      3s      4s      5s      6s      7s      8s
         |       |       |       |       |       |       |       |       |
Gossip:  ✓       ✓       ✓       X       .       .       .       .       .
         
Phi:     0       0       0       2       4       6       8       10      12
                                                 ^
                                            Threshold (8.0)
                                            Node marked FAILED
                                            
Events:  -       -       -       -       -       -    NodeFailed propagated
         
Registry:-       -       -       -       -       -    Worker removed from pool
         
Clients: -       -       -       -       -       -    Requests route elsewhere
```

**Total time to full recovery**: ~6-8 seconds with default settings

### Partition Detection Timeline

```
Time:    0s          5s          10s         15s         20s
         |           |           |           |           |
         Partition occurs
         |
         Side A can't reach Side B
         Side B can't reach Side A
         |
         Both sides mark other as "suspect"
                     |
                     Multiple nodes confirm partition
                                 |
                                 PartitionDetected event
                                             |
                                             Both sides operate independently
                                                         |
                                                         Partition heals
                                                         Gossip merges views
```

**Detection time**: 10-15 seconds  
**Recovery time**: 5-10 seconds after partition heals

## Retry Strategies

### Automatic Retry

```rust
use tokio::time::{sleep, Duration};

async fn call_with_retry<T>(
    f: impl Fn() -> Pin<Box<dyn Future<Output = Result<T>>>>,
    max_retries: usize,
) -> Result<T> {
    let mut retries = 0;
    
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if retries < max_retries => {
                retries += 1;
                log::warn!("Retry {}/{} after error: {}", retries, max_retries, e);
                
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(retries as u32));
                sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}

// Usage
let result = call_with_retry(
    || Box::pin(worker.call("compute", data.clone())),
    3
).await?;
```

### Failover to Different Worker

```rust
async fn call_with_failover(
    registry: Arc<WorkerRegistry>,
    method: &str,
    data: Vec<u8>,
    max_attempts: usize,
) -> Result<Response> {
    let mut attempted_workers = HashSet::new();
    
    for attempt in 0..max_attempts {
        // Select worker we haven't tried yet
        let worker = loop {
            let w = registry.select_worker(Some("role=worker")).await?;
            if !attempted_workers.contains(&w.id) {
                break w;
            }
            
            if attempted_workers.len() >= registry.worker_count().await {
                return Err(anyhow::anyhow!("All workers failed"));
            }
        };
        
        attempted_workers.insert(worker.id);
        
        log::info!("Attempt {}: trying worker {}", attempt + 1, worker.label);
        
        match worker.call(method, data.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                log::warn!("Worker {} failed: {}", worker.label, e);
                continue;
            }
        }
    }
    
    Err(anyhow::anyhow!("Failed after {} attempts", max_attempts))
}
```

### Circuit Breaker

Prevent cascading failures by temporarily stopping requests to failed nodes:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
enum CircuitState {
    Closed,       // Normal operation
    Open,         // Failing, reject requests
    HalfOpen,     // Testing recovery
}

struct CircuitBreaker {
    states: Arc<RwLock<HashMap<Uuid, CircuitState>>>,
    failure_threshold: usize,
    timeout: Duration,
}

impl CircuitBreaker {
    async fn call<T>(
        &self,
        worker_id: Uuid,
        f: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        let state = self.states.read().await
            .get(&worker_id)
            .cloned()
            .unwrap_or(CircuitState::Closed);
        
        match state {
            CircuitState::Open => {
                // Circuit open, reject immediately
                Err(anyhow::anyhow!("Circuit breaker open for worker {}", worker_id))
            }
            CircuitState::HalfOpen | CircuitState::Closed => {
                match f.await {
                    Ok(result) => {
                        // Success, close circuit
                        self.states.write().await.insert(worker_id, CircuitState::Closed);
                        Ok(result)
                    }
                    Err(e) => {
                        // Failure, open circuit
                        self.states.write().await.insert(worker_id, CircuitState::Open);
                        
                        // Schedule transition to half-open
                        let states = self.states.clone();
                        let timeout = self.timeout;
                        tokio::spawn(async move {
                            sleep(timeout).await;
                            states.write().await.insert(worker_id, CircuitState::HalfOpen);
                        });
                        
                        Err(e)
                    }
                }
            }
        }
    }
}
```

## Partition Handling

### Split-Brain Prevention

**Problem**: During partition, both sides may accept writes, leading to conflicts.

**Solution 1**: Majority quorum

```rust
async fn handle_partition_with_quorum(
    cluster: Arc<ClusterMembership>,
    total_nodes: usize,
) -> Result<()> {
    let visible_nodes = cluster.visible_nodes().await.len();
    let majority = total_nodes / 2 + 1;
    
    if visible_nodes < majority {
        log::error!("Lost majority quorum ({}/{}), entering read-only mode",
            visible_nodes, total_nodes);
        
        // Enter read-only mode
        set_read_only(true).await;
        
        // Wait for partition to heal
        loop {
            sleep(Duration::from_secs(5)).await;
            let current = cluster.visible_nodes().await.len();
            
            if current >= majority {
                log::info!("Regained quorum, resuming writes");
                set_read_only(false).await;
                break;
            }
        }
    }
    
    Ok(())
}
```

**Solution 2**: Designated leader

```rust
// Only one node (leader) accepts writes
async fn handle_partition_with_leader(
    cluster: Arc<ClusterMembership>,
    leader_id: Uuid,
) -> Result<()> {
    let my_id = cluster.local_node_id();
    
    if my_id == leader_id {
        // I'm the leader, check if I can reach majority
        if !can_reach_majority(&cluster).await {
            log::error!("Leader lost majority, stepping down");
            set_read_only(true).await;
        }
    } else {
        // I'm not the leader, check if I can reach leader
        if !can_reach_node(&cluster, leader_id).await {
            log::error!("Lost connection to leader, entering read-only mode");
            set_read_only(true).await;
        }
    }
    
    Ok(())
}
```

### Partition Recovery

When partition heals, nodes must reconcile state:

```rust
async fn handle_partition_recovery(
    cluster: Arc<ClusterMembership>,
) -> Result<()> {
    let mut events = cluster.subscribe();
    
    while let Some(event) = events.recv().await {
        if let ClusterEvent::PartitionHealed = event {
            log::info!("Partition healed, reconciling state");
            
            // Re-sync cluster state
            cluster.resync().await?;
            
            // Reconcile application state
            reconcile_application_state().await?;
            
            // Resume normal operation
            set_read_only(false).await;
            
            log::info!("Partition recovery complete");
        }
    }
    
    Ok(())
}

async fn reconcile_application_state() -> Result<()> {
    // Application-specific reconciliation logic
    // Examples:
    // - Compare vector clocks
    // - Merge CRDTs
    // - Apply conflict resolution rules
    // - Manual operator intervention
    
    Ok(())
}
```

## Client-Side Handling

### Transparent Failover

Clients should automatically failover to healthy workers:

```rust
// Client implementation with automatic failover
struct ResilientClient {
    registry: Arc<WorkerRegistry>,
    client: Arc<ClusterClient>,
}

impl ResilientClient {
    async fn call(&self, method: &str, data: Vec<u8>) -> Result<Response> {
        const MAX_ATTEMPTS: usize = 3;
        
        for attempt in 1..=MAX_ATTEMPTS {
            // Get healthy worker
            let worker = match self.registry.select_worker(Some("role=worker")).await {
                Ok(w) => w,
                Err(e) if attempt < MAX_ATTEMPTS => {
                    log::warn!("No workers available, retrying...");
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            
            // Get pooled connection
            let conn = self.connection_pool.get_or_connect(worker.addr).await?;
            
            // Make request
            match conn.call(method, data.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    log::warn!("Worker {} failed (attempt {}): {}", 
                        worker.label, attempt, e);
                    
                    // Mark worker as potentially failed
                    self.registry.report_failure(worker.id).await;
                    
                    if attempt < MAX_ATTEMPTS {
                        sleep(Duration::from_millis(100 * attempt as u64)).await;
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("All attempts failed"))
    }
}
```

### Request Hedging

Send duplicate requests to multiple workers, use first response:

```rust
async fn hedged_call(
    registry: Arc<WorkerRegistry>,
    method: &str,
    data: Vec<u8>,
    hedge_after: Duration,
) -> Result<Response> {
    let worker1 = registry.select_worker(Some("role=worker")).await?;
    
    // Start first request
    let req1 = worker1.call(method, data.clone());
    
    tokio::select! {
        result = req1 => result,
        _ = sleep(hedge_after) => {
            // First request taking too long, send hedge request
            log::info!("Hedging request to second worker");
            
            let worker2 = registry.select_worker(Some("role=worker")).await?;
            let req2 = worker2.call(method, data.clone());
            
            // Return whichever completes first
            tokio::select! {
                result = req1 => result,
                result = req2 => result,
            }
        }
    }
}
```

## Monitoring Failures

### Track Failure Metrics

```rust
struct FailureMetrics {
    node_failures: Counter,
    partition_count: Counter,
    retry_count: Counter,
    circuit_breaks: Counter,
}

async fn monitor_failures(cluster: Arc<ClusterMembership>) {
    let mut events = cluster.subscribe();
    
    while let Some(event) = events.recv().await {
        match event {
            ClusterEvent::NodeFailed(node) => {
                metrics::increment_counter!("cluster.node_failures");
                log::error!("Node {} failed", node.id);
                
                // Alert if critical worker
                if node.tags.get("critical") == Some(&"true".to_string()) {
                    alert_ops_team(&format!("Critical node {} failed", node.id));
                }
            }
            ClusterEvent::PartitionDetected(_) => {
                metrics::increment_counter!("cluster.partitions");
                alert_ops_team("Network partition detected");
            }
            _ => {}
        }
    }
}
```

### Health Dashboard

```rust
async fn health_dashboard(registry: Arc<WorkerRegistry>) -> String {
    let workers = registry.workers().await;
    let total = workers.len();
    let healthy = workers.iter().filter(|w| w.is_healthy()).count();
    let degraded = workers.iter().filter(|w| w.is_degraded()).count();
    let failed = total - healthy - degraded;
    
    format!(
        "Cluster Health:\n\
         Total Workers: {}\n\
         Healthy: {} ({}%)\n\
         Degraded: {} ({}%)\n\
         Failed: {} ({}%)\n",
        total,
        healthy, (healthy * 100 / total),
        degraded, (degraded * 100 / total),
        failed, (failed * 100 / total)
    )
}
```

## Best Practices

### 1. Design for Failure

```rust
// Assume failures will happen
// ✅ Good: Handle failures gracefully
async fn process(data: Vec<u8>) -> Result<Response> {
    match call_worker(data.clone()).await {
        Ok(response) => Ok(response),
        Err(e) => {
            log::error!("Worker call failed: {}", e);
            fallback_processing(data).await
        }
    }
}

// ❌ Bad: No failure handling
async fn process(data: Vec<u8>) -> Result<Response> {
    call_worker(data).await  // Will panic/error if worker fails
}
```

### 2. Set Appropriate Timeouts

```rust
// ✅ Good: Timeout prevents hanging
let result = tokio::time::timeout(
    Duration::from_secs(5),
    worker.call("compute", data)
).await??;

// ❌ Bad: No timeout, could hang forever
let result = worker.call("compute", data).await?;
```

### 3. Implement Idempotency

```rust
// ✅ Good: Idempotent operations safe to retry
#[rpc_trait]
pub trait ComputeService {
    async fn process(&self, request_id: Uuid, data: Vec<u8>) -> Result<Response>;
    //                      ^^^^^^^^^^^^ request ID makes it idempotent
}

// Check if already processed
if let Some(cached) = self.check_cache(request_id).await {
    return Ok(cached);
}
```

### 4. Monitor Everything

```rust
// Track all failure types
metrics::increment_counter!("failures.node_crash");
metrics::increment_counter!("failures.timeout");
metrics::increment_counter!("failures.partition");
metrics::gauge!("cluster.healthy_nodes", healthy_count as f64);
```

### 5. Test Failure Scenarios

```rust
#[tokio::test]
async fn test_worker_failure() {
    // Start cluster
    let (director, workers) = setup_cluster().await;
    
    // Kill one worker
    workers[0].shutdown().await;
    
    // Verify requests still succeed
    let client = ResilientClient::new(director.registry());
    let result = client.call("compute", vec![1, 2, 3]).await;
    assert!(result.is_ok());
}
```

## Next Steps

- **[Discovery](discovery.md)** - Understand how nodes discover failures
- **[Health Checking](health.md)** - Learn about Phi Accrual detection
- **[Production Guide](../advanced/production.md)** - Deploy resilient clusters

## References

- [Fallacies of Distributed Computing](https://en.wikipedia.org/wiki/Fallacies_of_distributed_computing) - Common mistakes
- [CAP Theorem](https://en.wikipedia.org/wiki/CAP_theorem) - Consistency vs Availability trade-offs
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html) - Martin Fowler's article
