# Health Checking

RpcNet uses the **Phi Accrual Failure Detector** algorithm for accurate and adaptive health checking. This chapter explains how RpcNet determines which nodes are healthy and when to mark them as failed.

## The Problem with Binary Health Checks

Traditional health checks use binary logic:

```
if (ping_timeout):
    node_is_failed = True
else:
    node_is_healthy = True
```

**Problems**:
1. **Fixed threshold**: 500ms timeout doesn't adapt to network conditions
2. **False positives**: Temporary slowdown triggers failure
3. **False negatives**: Slow node stays "healthy" until timeout
4. **No confidence**: Can't express "probably failed" vs "definitely failed"

## Phi Accrual Solution

The Phi Accrual algorithm provides a **continuous suspicion level** instead of binary alive/dead:

```
Phi Value (Φ) = Suspicion Level

Φ = 0     → Node is responding normally
Φ = 5     → Moderate suspicion (50% chance failed)
Φ = 8     → High suspicion (97.7% chance failed) ← Typical threshold
Φ = 10    → Very high suspicion (99.99% chance failed)
Φ = 15+   → Almost certainly failed
```

### How It Works

**1. Track Heartbeat History**

```rust
struct HeartbeatHistory {
    intervals: Vec<Duration>,  // Last N intervals between heartbeats
    last_heartbeat: Instant,   // When we last heard from node
}
```

**2. Calculate Expected Interval**

```rust
fn mean_interval(&self) -> Duration {
    self.intervals.iter().sum::<Duration>() / self.intervals.len()
}

fn std_deviation(&self) -> Duration {
    let mean = self.mean_interval();
    let variance = self.intervals
        .iter()
        .map(|&interval| {
            let diff = interval.as_secs_f64() - mean.as_secs_f64();
            diff * diff
        })
        .sum::<f64>() / self.intervals.len() as f64;
    
    Duration::from_secs_f64(variance.sqrt())
}
```

**3. Compute Phi**

```rust
fn phi(&self) -> f64 {
    let now = Instant::now();
    let time_since_last = now.duration_since(self.last_heartbeat);
    let mean = self.mean_interval();
    let std_dev = self.std_deviation();
    
    // How many standard deviations away is current delay?
    let z_score = (time_since_last.as_secs_f64() - mean.as_secs_f64()) 
                  / std_dev.as_secs_f64();
    
    // Convert to phi (log probability)
    -z_score.ln() / 2.0_f64.ln()
}
```

**4. Determine Failure**

```rust
const PHI_THRESHOLD: f64 = 8.0;  // Configurable

if phi() > PHI_THRESHOLD {
    mark_node_as_failed();
}
```

## Visualization

### Example 1: Healthy Node

```
Heartbeats arrive regularly every ~1 second:

Time (s):    0    1    2    3    4    5    6    7    8
Heartbeat:   ✓    ✓    ✓    ✓    ✓    ✓    ✓    ✓    ✓
Phi:         0    0    0    0    0    0    0    0    0

Status: Healthy (Φ = 0)
```

### Example 2: Temporary Network Glitch

```
Heartbeats delayed but node recovers:

Time (s):    0    1    2    3    4    5    6    7    8
Heartbeat:   ✓    ✓    ✓    .    .    ✓    ✓    ✓    ✓
Phi:         0    0    0    2    5    2    0    0    0
                              ▲
                              Elevated but below threshold

Status: Suspect briefly, but recovers (no failure declared)
```

### Example 3: Actual Failure

```
Heartbeats stop after node crashes:

Time (s):    0    1    2    3    4    5    6    7    8
Heartbeat:   ✓    ✓    ✓    X    .    .    .    .    .
Phi:         0    0    0    2    5    8    11   14   17
                                   ▲
                                   Exceeds threshold → FAILED

Status: Failed (Φ = 8+)
```

## Adaptive Behavior

Phi Accrual adapts to network conditions automatically:

### Stable Network

```
History: [1.0s, 1.0s, 1.0s, 1.0s, 1.0s]
Mean: 1.0s
Std Dev: 0.0s (very predictable)

Current delay: 1.5s
Phi: 8.0 → FAILURE (unusual for this stable network)
```

### Variable Network

```
History: [0.8s, 1.2s, 0.9s, 1.4s, 1.0s]
Mean: 1.06s
Std Dev: 0.24s (more variable)

Current delay: 1.5s
Phi: 3.2 → HEALTHY (normal variation)
```

**Key insight**: Same 1.5s delay is interpreted differently based on historical patterns.

## RpcNet Implementation

### Configuration

```rust
use rpcnet::cluster::{ClusterConfig, HealthCheckConfig};
use std::time::Duration;

let health_config = HealthCheckConfig::default()
    .with_interval(Duration::from_secs(1))        // Check every 1 second
    .with_phi_threshold(8.0)                       // Suspicion threshold
    .with_history_size(100)                        // Track last 100 intervals
    .with_min_std_deviation(Duration::from_millis(50)); // Min variation

let cluster_config = ClusterConfig::default()
    .with_health_check(health_config);

let cluster = ClusterMembership::new(cluster_config).await?;
```

### Monitoring Health

```rust
// Subscribe to health events
let mut events = cluster.subscribe();

while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeSuspect(node, phi) => {
            println!("Node {} suspect (Φ = {:.2})", node.id, phi);
        }
        ClusterEvent::NodeFailed(node) => {
            println!("Node {} failed (Φ exceeded threshold)", node.id);
        }
        ClusterEvent::NodeRecovered(node) => {
            println!("Node {} recovered (Φ back to normal)", node.id);
        }
        _ => {}
    }
}
```

### Custom Phi Threshold

Different thresholds for different applications:

```rust
// Conservative (fewer false positives, slower detection)
.with_phi_threshold(10.0)  // 99.99% confidence

// Aggressive (faster detection, more false positives)
.with_phi_threshold(5.0)   // 50% confidence

// Recommended default
.with_phi_threshold(8.0)   // 97.7% confidence
```

## Choosing Phi Threshold

| Threshold | Confidence | False Positive Rate | Detection Time | Use Case |
|-----------|-----------|---------------------|----------------|----------|
| **3.0** | 12.5% | Very High | Very Fast | Testing only |
| **5.0** | 50% | High | Fast | Aggressive failover |
| **8.0** | 97.7% | Low | Moderate | **Recommended** |
| **10.0** | 99.99% | Very Low | Slower | Critical systems |
| **12.0** | 99.9999% | Extremely Low | Slow | High-latency networks |

### Threshold Selection Guide

**Low threshold (3-5)** if:
- Fast failover is critical
- False positives are acceptable
- Network is very stable

**Medium threshold (6-9)** if:
- Balance between speed and accuracy
- Typical production environments
- **Recommended for most use cases**

**High threshold (10+)** if:
- False positives are very costly
- Network has high variance
- Graceful degradation preferred over fast failover

## Integration with SWIM

Phi Accrual works alongside SWIM's failure detection:

```
┌─────────────────────────────────────────────────────┐
│                   SWIM Protocol                      │
│                                                      │
│  1. Gossip → Heartbeats to Phi Accrual              │
│  2. Phi Accrual → Computes suspicion level          │
│  3. Φ > threshold → Mark node as Suspect            │
│  4. Indirect probes → Verify with other nodes       │
│  5. Multiple confirmations → Mark node as Failed    │
│  6. Gossip spreads failure → All nodes updated      │
└─────────────────────────────────────────────────────┘
```

**Process**:

1. **Regular operation**: Nodes exchange gossip messages (heartbeats)
2. **Phi calculation**: Each heartbeat updates Phi Accrual history
3. **Suspicion**: When Φ exceeds threshold, node marked Suspect
4. **Verification**: SWIM performs indirect probes to confirm
5. **Failure declaration**: Multiple nodes agree → Node marked Failed
6. **Recovery**: If heartbeats resume, Φ drops and node marked Alive again

## Performance Characteristics

### Computational Overhead

```rust
// Phi calculation per node per check:
// - Mean: O(1) with running average
// - Std dev: O(1) with running variance
// - Phi: O(1) math operations

// Total overhead: ~500ns per node per health check
```

**For 100 nodes checked every 1 second**: 0.05ms total CPU time (negligible)

### Memory Overhead

```rust
struct NodeHealth {
    intervals: VecDeque<Duration>,  // 100 entries × 16 bytes = 1.6 KB
    last_heartbeat: Instant,        // 16 bytes
    running_mean: Duration,         // 16 bytes
    running_variance: f64,          // 8 bytes
}

// Total per node: ~1.7 KB
```

**For 100 nodes**: ~170 KB memory (negligible)

### Detection Time

Measured time from actual failure to detection:

| Network Stability | Heartbeat Interval | Phi Threshold | Detection Time |
|-------------------|-------------------|---------------|----------------|
| Stable (σ=10ms) | 1s | 8.0 | 2-3s |
| Variable (σ=200ms) | 1s | 8.0 | 4-6s |
| Unstable (σ=500ms) | 1s | 8.0 | 8-12s |

**Tuning for faster detection**: Reduce heartbeat interval (e.g., 500ms)

## Comparison to Alternatives

### vs Fixed Timeout

```
Fixed Timeout:
  ✗ Doesn't adapt to network conditions
  ✗ Binary alive/dead (no confidence)
  ✓ Simple implementation

Phi Accrual:
  ✓ Adapts automatically
  ✓ Continuous suspicion level
  ✓ Fewer false positives
  ✗ More complex
```

### vs Heartbeat Count

```
Heartbeat Count (miss N in a row):
  ✗ Slow detection (N × interval)
  ✗ Doesn't account for network variance
  ✓ Simple logic

Phi Accrual:
  ✓ Faster detection
  ✓ Accounts for network patterns
  ✓ Adaptive threshold
```

### vs Gossip Only

```
Gossip Only (no Phi):
  ✗ Hard threshold (suspect → failed)
  ✗ Doesn't adapt to network
  ✓ Simpler protocol

Gossip + Phi:
  ✓ Smooth suspicion curve
  ✓ Adapts to network conditions
  ✓ More accurate detection
```

## Best Practices

### 1. Tune for Your Network

```rust
// Measure your network characteristics first
async fn measure_network_latency() -> (Duration, Duration) {
    let mut latencies = Vec::new();
    
    for _ in 0..100 {
        let start = Instant::now();
        ping_peer().await.unwrap();
        latencies.push(start.elapsed());
    }
    
    let mean = latencies.iter().sum::<Duration>() / latencies.len();
    let variance = latencies.iter()
        .map(|&d| (d.as_secs_f64() - mean.as_secs_f64()).powi(2))
        .sum::<f64>() / latencies.len() as f64;
    let std_dev = Duration::from_secs_f64(variance.sqrt());
    
    println!("Network latency: {:.2?} ± {:.2?}", mean, std_dev);
    (mean, std_dev)
}

// Then configure accordingly
let (mean, std_dev) = measure_network_latency().await;
let health_config = HealthCheckConfig::default()
    .with_interval(mean * 2)          // Check at 2× mean latency
    .with_phi_threshold(8.0)
    .with_min_std_deviation(std_dev);
```

### 2. Monitor Phi Values

```rust
// Log phi values to understand patterns
async fn monitor_phi_values(cluster: Arc<ClusterMembership>) {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        for node in cluster.nodes().await {
            let phi = cluster.phi(node.id).await.unwrap_or(0.0);
            
            if phi > 5.0 {
                log::warn!("Node {} phi elevated: {:.2}", node.id, phi);
            }
            
            metrics::gauge!("cluster.node.phi", phi, "node" => node.id.to_string());
        }
    }
}
```

### 3. Handle Suspicion State

```rust
// Don't immediately fail on suspicion - investigate first
let mut events = cluster.subscribe();

while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeSuspect(node, phi) => {
            log::warn!("Node {} suspect (Φ = {:.2}), investigating...", node.id, phi);
            
            // Trigger additional checks
            tokio::spawn(async move {
                if let Err(e) = verify_node_health(&node).await {
                    log::error!("Node {} verification failed: {}", node.id, e);
                }
            });
        }
        ClusterEvent::NodeFailed(node) => {
            log::error!("Node {} failed, removing from pool", node.id);
            remove_from_worker_pool(node.id).await;
        }
        _ => {}
    }
}
```

### 4. Adjust History Size

```rust
// Larger history = more stable, slower adaptation
.with_history_size(200)  // For very stable networks

// Smaller history = faster adaptation to changes
.with_history_size(50)   // For dynamic networks

// Default (recommended)
.with_history_size(100)
```

### 5. Set Minimum Standard Deviation

```rust
// Prevent division by zero and overly sensitive detection
.with_min_std_deviation(Duration::from_millis(50))

// Higher min = less sensitive to small variations
.with_min_std_deviation(Duration::from_millis(100))
```

## Troubleshooting

### False Positives (Node marked failed but is alive)

**Symptoms**:
- Nodes frequently marked failed and recovered
- Phi threshold exceeded during normal operation

**Debug**:
```rust
// Log phi values and intervals
for node in cluster.nodes().await {
    let phi = cluster.phi(node.id).await.unwrap_or(0.0);
    let history = cluster.heartbeat_history(node.id).await;
    println!("Node {}: Φ = {:.2}, intervals = {:?}", node.id, phi, history);
}
```

**Solutions**:
- Increase phi threshold (8.0 → 10.0)
- Increase heartbeat interval to match network latency
- Increase min_std_deviation for variable networks

### Slow Detection (Failures take too long to detect)

**Symptoms**:
- Nodes crash but stay marked alive for minutes
- Requests keep routing to failed nodes

**Debug**:
```rust
// Measure actual detection time
let failure_time = Instant::now();
// ... node fails ...
let detection_time = cluster.wait_for_failure(node_id).await;
println!("Detection took: {:?}", detection_time.duration_since(failure_time));
```

**Solutions**:
- Decrease phi threshold (8.0 → 6.0)
- Decrease heartbeat interval (1s → 500ms)
- Decrease suspicion timeout

### Memory Growth

**Symptoms**:
- Memory usage grows over time
- History buffers not bounded

**Debug**:
```rust
// Check history sizes
for node in cluster.nodes().await {
    let history = cluster.heartbeat_history(node.id).await;
    println!("Node {}: {} intervals tracked", node.id, history.len());
}
```

**Solutions**:
- Ensure history_size is set (default: 100)
- Verify old entries are removed
- Check for node ID leaks

## Advanced Topics

### Combining Multiple Detectors

Use Phi Accrual for heartbeats AND application-level health:

```rust
struct CompositeHealthCheck {
    phi_detector: PhiAccrualDetector,
    app_health: Arc<Mutex<HashMap<Uuid, bool>>>,
}

impl CompositeHealthCheck {
    async fn is_healthy(&self, node_id: Uuid) -> bool {
        // Both phi and application health must be good
        let phi = self.phi_detector.phi(node_id);
        let app_healthy = self.app_health.lock().await.get(&node_id).copied().unwrap_or(false);
        
        phi < PHI_THRESHOLD && app_healthy
    }
}
```

### Weighted Phi Thresholds

Different thresholds for different node types:

```rust
fn get_phi_threshold(node: &Node) -> f64 {
    match node.tags.get("criticality") {
        Some("high") => 10.0,    // Very conservative for critical nodes
        Some("low") => 6.0,      // Aggressive for non-critical
        _ => 8.0,                // Default
    }
}
```

## Next Steps

- **[Failures](failures.md)** - Handle node failures and partitions
- **[Discovery](discovery.md)** - How nodes discover each other via gossip

## References

- [Phi Accrual Paper](https://citeseerx.ist.psu.edu/document?repid=rep1&type=pdf&doi=babf246cf6753ad12ce97ae47e64c9d4ff85c6f7) - Original algorithm
- [Cassandra Failure Detection](https://cassandra.apache.org/doc/latest/cassandra/architecture/failure_detection.html) - Production implementation
- [Akka Cluster Phi](https://doc.akka.io/docs/akka/current/typed/failure-detector.html) - Akka's usage
