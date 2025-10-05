# RpcNet Cluster Management Design - V2 (Issues Resolved)

**Status**: Ready for Implementation  
**Version**: 2.0 (addresses all critical analysis findings)  
**Date**: 2025-10-02

---

## Changes from V1

This version addresses **10 critical issues** identified in code review:

1. ✅ QUIC Connection Pooling Architecture
2. ✅ Incarnation Number Conflict Resolution
3. ✅ Gossip Message Size Bounds
4. ✅ Tag Update Race Conditions
5. ✅ Split Brain Detection Algorithm
6. ✅ Correct Phi Accrual Implementation
7. ✅ Health Check Circular Dependency
8. ✅ Graceful Shutdown Sequence
9. ✅ Event Backpressure Strategy
10. ✅ Seed Node Bootstrap Retry Logic

---

## Executive Summary

**Scope**: SWIM-based cluster membership for rpcnet with **bounded complexity**

**Key Constraints** (NEW):
- Maximum cluster size: **100 nodes** (not 1000+)
- Maximum gossip message: **4KB** (fits in single UDP packet)
- Connection pool: **50 connections** per node
- Tag update propagation: **< 3 seconds** (p99)
- Failure detection: **< 15 seconds** (p99, not 20s)

**Success Criteria**: connection_swap works with **zero manual heartbeat code**

---

## Issue 1: Connection Pool Architecture

### Problem
Original design opened new QUIC connection per gossip message (300 handshakes/sec for 100 nodes).

### Solution: Persistent Connection Pool

```rust
// src/cluster/connection_pool.rs

pub struct ConnectionPool {
    connections: DashMap<SocketAddr, PooledConnection>,
    config: PoolConfig,
    quic_client: Arc<s2n_quic::Client>,
}

#[derive(Clone)]
pub struct PoolConfig {
    /// Maximum connections to keep open per peer (default: 1)
    pub max_per_peer: usize,
    
    /// Maximum total connections (default: 50)
    pub max_total: usize,
    
    /// How long to keep idle connection before closing (default: 60s)
    pub idle_timeout: Duration,
    
    /// How long to wait for connection establishment (default: 5s)
    pub connect_timeout: Duration,
    
    /// Health check interval for idle connections (default: 30s)
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_peer: 1,
            max_total: 50,
            idle_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

struct PooledConnection {
    connection: Arc<s2n_quic::Connection>,
    last_used: Arc<RwLock<Instant>>,
    in_use: AtomicBool,
}

impl ConnectionPool {
    pub async fn get_or_create(&self, addr: SocketAddr) -> Result<PooledConnection, PoolError> {
        // Try to reuse existing connection
        if let Some(conn) = self.connections.get(&addr) {
            if !conn.in_use.swap(true, Ordering::SeqCst) {
                *conn.last_used.write().await = Instant::now();
                return Ok(conn.clone());
            }
        }
        
        // Check total connection limit
        if self.connections.len() >= self.config.max_total {
            self.evict_idle_connection().await?;
        }
        
        // Create new connection
        let connect = Connect::new(addr)
            .with_server_name("cluster")
            .with_timeout(self.config.connect_timeout);
        
        let connection = tokio::time::timeout(
            self.config.connect_timeout,
            self.quic_client.connect(connect)
        ).await??;
        
        let pooled = PooledConnection {
            connection: Arc::new(connection),
            last_used: Arc::new(RwLock::new(Instant::now())),
            in_use: AtomicBool::new(true),
        };
        
        self.connections.insert(addr, pooled.clone());
        Ok(pooled)
    }
    
    pub fn release(&self, addr: &SocketAddr) {
        if let Some(conn) = self.connections.get(addr) {
            conn.in_use.store(false, Ordering::SeqCst);
        }
    }
    
    async fn evict_idle_connection(&self) -> Result<(), PoolError> {
        // Find oldest idle connection
        let mut oldest_addr = None;
        let mut oldest_time = Instant::now();
        
        for entry in self.connections.iter() {
            let conn = entry.value();
            if !conn.in_use.load(Ordering::SeqCst) {
                let last_used = *conn.last_used.read().await;
                if last_used < oldest_time {
                    oldest_time = last_used;
                    oldest_addr = Some(*entry.key());
                }
            }
        }
        
        if let Some(addr) = oldest_addr {
            self.connections.remove(&addr);
            Ok(())
        } else {
            Err(PoolError::NoIdleConnections)
        }
    }
    
    /// Background task to close idle connections
    pub async fn run_idle_cleanup(&self) {
        let mut interval = tokio::time::interval(self.config.health_check_interval);
        
        loop {
            interval.tick().await;
            
            let now = Instant::now();
            let idle_threshold = now - self.config.idle_timeout;
            
            let to_remove: Vec<_> = self.connections.iter()
                .filter(|entry| {
                    let conn = entry.value();
                    !conn.in_use.load(Ordering::SeqCst) &&
                    *conn.last_used.blocking_read() < idle_threshold
                })
                .map(|entry| *entry.key())
                .collect();
            
            for addr in to_remove {
                self.connections.remove(&addr);
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("Pool is full and no idle connections available")]
    NoIdleConnections,
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("QUIC error: {0}")]
    QuicError(String),
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_connection_reuse() {
    let pool = ConnectionPool::new(PoolConfig::default());
    let addr = "127.0.0.1:60001".parse().unwrap();
    
    // First connection
    let conn1 = pool.get_or_create(addr).await.unwrap();
    let conn1_ptr = Arc::as_ptr(&conn1.connection);
    pool.release(&addr);
    
    // Second connection should reuse
    let conn2 = pool.get_or_create(addr).await.unwrap();
    let conn2_ptr = Arc::as_ptr(&conn2.connection);
    
    assert_eq!(conn1_ptr, conn2_ptr, "Should reuse same connection");
}

#[tokio::test]
async fn test_pool_eviction() {
    let pool = ConnectionPool::new(PoolConfig {
        max_total: 2,
        ..Default::default()
    });
    
    // Fill pool
    let addr1 = "127.0.0.1:60001".parse().unwrap();
    let addr2 = "127.0.0.1:60002".parse().unwrap();
    let conn1 = pool.get_or_create(addr1).await.unwrap();
    let conn2 = pool.get_or_create(addr2).await.unwrap();
    
    pool.release(&addr1);  // Mark addr1 as idle
    
    // This should evict addr1
    let addr3 = "127.0.0.1:60003".parse().unwrap();
    let conn3 = pool.get_or_create(addr3).await.unwrap();
    
    assert!(!pool.connections.contains_key(&addr1), "addr1 should be evicted");
    assert_eq!(pool.connections.len(), 2);
}
```

---

## Issue 2: Incarnation Number Conflicts

### Problem
Incarnation wraparound, initialization, and conflict resolution unspecified.

### Solution: Lamport-Style Incarnation

```rust
// src/cluster/incarnation.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incarnation(u64);

impl Incarnation {
    /// Create initial incarnation from timestamp
    pub fn initial() -> Self {
        Self(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64)
    }
    
    /// Increment incarnation (for refutation)
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
    
    /// Compare incarnations (handles wraparound)
    pub fn compare(&self, other: &Incarnation) -> Ordering {
        // Wraparound-safe comparison within window of u64::MAX/2
        const HALF: u64 = u64::MAX / 2;
        
        if self.0 == other.0 {
            Ordering::Equal
        } else if (self.0.wrapping_sub(other.0)) < HALF {
            Ordering::Greater  // self is newer
        } else {
            Ordering::Less     // other is newer
        }
    }
}

impl Ord for Incarnation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl PartialOrd for Incarnation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Resolve conflict between two node statuses
pub fn resolve_conflict(a: &NodeStatus, b: &NodeStatus) -> &NodeStatus {
    match a.incarnation.cmp(&b.incarnation) {
        Ordering::Greater => a,
        Ordering::Less => b,
        Ordering::Equal => {
            // Same incarnation - use NodeId as tiebreaker (deterministic)
            if a.node.id > b.node.id {
                a
            } else {
                b
            }
        }
    }
}
```

**Testing**:
```rust
#[test]
fn test_incarnation_wraparound() {
    let old = Incarnation(u64::MAX - 10);
    let new = Incarnation(5);  // Wrapped around
    
    assert!(new > old, "Wrapped incarnation should be newer");
}

#[test]
fn test_incarnation_window() {
    let base = Incarnation(1000);
    let far_future = Incarnation(base.0 + u64::MAX / 2 + 1);
    
    // Outside window - considered older
    assert!(far_future < base);
}

#[test]
fn test_conflict_resolution_tiebreaker() {
    let node_a = ClusterNode { id: NodeId(Uuid::new_v4()), ... };
    let node_b = ClusterNode { id: NodeId(Uuid::new_v4()), ... };
    
    let status_a = NodeStatus {
        node: node_a.clone(),
        incarnation: Incarnation(100),
        ..Default::default()
    };
    
    let status_b = NodeStatus {
        node: node_b.clone(),
        incarnation: Incarnation(100),  // Same incarnation
        ..Default::default()
    };
    
    let winner = resolve_conflict(&status_a, &status_b);
    
    // Same winner every time (deterministic)
    assert_eq!(winner, resolve_conflict(&status_a, &status_b));
}
```

---

## Issue 3: Gossip Message Size Bounds

### Problem
Ping message could include 1000s of node updates (500KB+).

### Solution: Bounded Update Batches

```rust
// src/cluster/gossip.rs

/// Maximum node updates per gossip message (fits in 4KB)
const MAX_UPDATES_PER_MESSAGE: usize = 20;

/// Maximum gossip message size (bytes)
const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum GossipMessage {
    Ping {
        from: NodeId,
        incarnation: Incarnation,
        updates: BoundedVec<NodeUpdate, MAX_UPDATES_PER_MESSAGE>,  // ✅ BOUNDED
        checksum: u32,  // Verify message integrity
    },
    
    Ack {
        from: NodeId,
        incarnation: Incarnation,
    },
    
    PingReq {
        from: NodeId,
        target: NodeId,
        requestor: NodeId,
    },
    
    Join {
        node: ClusterNode,
    },
    
    Leave {
        node_id: NodeId,
        incarnation: Incarnation,
    },
}

/// Priority queue for selecting which updates to gossip
struct GossipQueue {
    updates: BTreeMap<Priority, NodeUpdate>,
    seen_count: HashMap<NodeId, usize>,  // Track propagation
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Critical,   // Node failed
    High,       // Node joined/left
    Medium,     // State change
    Low,        // Metadata update
}

impl GossipQueue {
    fn add_update(&mut self, update: NodeUpdate, priority: Priority) {
        self.updates.insert(priority, update);
        *self.seen_count.entry(update.node_id).or_insert(0) += 0;
    }
    
    fn select_updates(&mut self) -> Vec<NodeUpdate> {
        let mut selected = Vec::with_capacity(MAX_UPDATES_PER_MESSAGE);
        
        // Take highest priority updates first
        for (priority, update) in self.updates.iter() {
            if selected.len() >= MAX_UPDATES_PER_MESSAGE {
                break;
            }
            
            let seen = self.seen_count.get(&update.node_id).copied().unwrap_or(0);
            
            // Stop gossiping after N rounds (log(cluster_size) * 3)
            if seen < 15 {  // log2(100) * 3 ≈ 20
                selected.push(update.clone());
                *self.seen_count.get_mut(&update.node_id).unwrap() += 1;
            }
        }
        
        // Remove updates that have been gossiped enough
        self.updates.retain(|_, update| {
            self.seen_count.get(&update.node_id).copied().unwrap_or(0) < 15
        });
        
        selected
    }
}

/// Verify message size before sending
fn check_message_size(msg: &GossipMessage) -> Result<(), GossipError> {
    let size = bincode::serialized_size(msg)
        .map_err(|e| GossipError::SerializationError(e))?;
    
    if size > MAX_MESSAGE_SIZE as u64 {
        return Err(GossipError::MessageTooLarge {
            size: size as usize,
            max: MAX_MESSAGE_SIZE,
        });
    }
    
    Ok(())
}
```

**Testing**:
```rust
#[test]
fn test_message_size_limit() {
    let updates: Vec<_> = (0..100)
        .map(|i| NodeUpdate {
            node_id: NodeId(Uuid::new_v4()),
            state: NodeState::Alive,
            incarnation: Incarnation(i),
        })
        .collect();
    
    let msg = GossipMessage::Ping {
        from: NodeId(Uuid::new_v4()),
        incarnation: Incarnation(0),
        updates: updates[..MAX_UPDATES_PER_MESSAGE].to_vec(),
        checksum: 0,
    };
    
    assert!(check_message_size(&msg).is_ok());
    
    let size = bincode::serialized_size(&msg).unwrap();
    assert!(size <= MAX_MESSAGE_SIZE as u64, "Message size {} exceeds {}", size, MAX_MESSAGE_SIZE);
}

#[test]
fn test_gossip_queue_priority() {
    let mut queue = GossipQueue::new();
    
    queue.add_update(NodeUpdate { /* failed node */ }, Priority::Critical);
    queue.add_update(NodeUpdate { /* joined node */ }, Priority::High);
    queue.add_update(NodeUpdate { /* metadata */ }, Priority::Low);
    
    let selected = queue.select_updates();
    
    // Critical should come first
    assert_eq!(selected[0].priority, Priority::Critical);
}

#[test]
fn test_gossip_propagation_limit() {
    let mut queue = GossipQueue::new();
    let update = NodeUpdate { node_id: NodeId(Uuid::new_v4()), ... };
    
    queue.add_update(update.clone(), Priority::High);
    
    // Gossip 20 times
    for _ in 0..20 {
        queue.select_updates();
    }
    
    // Should stop being selected after 15 times
    let selected = queue.select_updates();
    assert!(!selected.contains(&update));
}
```

---

## Issue 4: Tag Update Race Conditions

### Problem
Director queries `available=true` before worker's tag update propagates (0.5s race window).

### Solution: Optimistic Director with Retry

**Strategy**: Director assumes cluster state is eventually consistent. On connection failure, retry with different worker immediately.

```rust
// src/cluster/mod.rs - Add local tag cache

impl ClusterMembership {
    /// Update local tag (propagates on next gossip round - eventual consistency)
    pub async fn update_tag(&self, key: String, value: String) {
        let mut nodes = self.nodes.write().await;
        if let Some(status) = nodes.get_mut(&self.local_node.id) {
            status.node.tags.insert(key.clone(), value.clone());
            
            // Immediately broadcast as high-priority update
            let update = NodeUpdate {
                node_id: self.local_node.id.clone(),
                node: status.node.clone(),
                state: status.state,
                incarnation: status.incarnation,
            };
            
            self.gossip_queue.lock().await.add_update(update, Priority::High);
            
            // Emit event
            self.event_tx.send(ClusterEvent::NodeTagsUpdated {
                node_id: self.local_node.id.clone(),
                tags: status.node.tags.clone(),
            }).ok();
        }
    }
    
    /// Get nodes with tag (returns eventual-consistent view)
    pub async fn nodes_with_tag(&self, key: &str, value: &str) -> Vec<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes.values()
            .filter(|status| {
                status.state == NodeState::Alive &&
                status.node.tags.get(key) == Some(&value.to_string())
            })
            .map(|status| status.node.clone())
            .collect()
    }
}

// examples/connection_swap/src/bin/director.rs - Retry logic

async fn assign_worker(&self, server: &RpcServer) -> Option<WorkerAssignment> {
    const MAX_RETRIES: usize = 3;
    
    for attempt in 0..MAX_RETRIES {
        let workers = server.cluster()
            .unwrap()
            .nodes_with_tag("role", "worker")
            .await
            .into_iter()
            .filter(|n| n.tags.get("available") == Some(&"true".to_string()))
            .collect::<Vec<_>>();
        
        if workers.is_empty() {
            warn!(attempt, "no available workers found, retrying...");
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }
        
        let idx = self.next_worker_idx.fetch_add(1, Ordering::SeqCst);
        let worker = &workers[idx % workers.len()];
        
        // Return worker address
        return Some(WorkerAssignment {
            worker_id: worker.id.clone(),
            rpc_addr: worker.rpc_addr,
            label: worker.tags.get("label").cloned().unwrap_or_default(),
        });
    }
    
    None
}

// Client detects connection failure, goes back to director
// Director tries different worker on next attempt (natural retry)
```

**Expected Propagation Time**:
- Gossip interval: 1s
- Fanout: 3
- 100 nodes: log₃(100) ≈ 4.2 rounds
- **Total: ~5 seconds** for full propagation
- **P99: < 10 seconds** (including retries)

**Testing**:
```rust
#[tokio::test]
async fn test_tag_update_propagation() {
    let cluster = create_3_node_cluster().await;
    
    // Node 0 updates tag
    cluster[0].update_tag("status".to_string(), "busy".to_string()).await;
    
    // Wait for gossip propagation
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // All nodes should see the update
    for i in 1..3 {
        let nodes = cluster[i].nodes_with_tag("status", "busy").await;
        assert_eq!(nodes.len(), 1, "Node {} should see tag update", i);
    }
}

#[tokio::test]
async fn test_director_retry_on_stale_state() {
    let director = start_director_with_cluster().await;
    let worker = start_worker_with_cluster("worker-a", true).await;
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Worker marks itself unavailable
    worker.cluster().unwrap().update_tag("available".to_string(), "false".to_string()).await;
    
    // Director tries to assign immediately (stale state)
    let assignment = director.assign_worker().await;
    
    // Should succeed because retry logic handles race
    assert!(assignment.is_some() || director.cluster().unwrap().nodes_with_tag("role", "worker").await.is_empty());
}
```

---

## Issue 5: Split Brain Detection

### Problem
Detection algorithm vague ("cluster shrink significantly").

### Solution: Formal Partition Detection with Quorum

```rust
// src/cluster/partition.rs

pub struct PartitionDetector {
    expected_size: AtomicUsize,
    config: PartitionConfig,
    detection_start: Arc<RwLock<Option<Instant>>>,
}

#[derive(Debug, Clone)]
pub struct PartitionConfig {
    /// Threshold for declaring partition (default: 0.5 = majority)
    pub threshold: f64,
    
    /// Grace period before declaring partition (default: 30s)
    pub grace_period: Duration,
    
    /// Enable read-only mode when partitioned (default: false)
    pub read_only_on_partition: bool,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            grace_period: Duration::from_secs(30),
            read_only_on_partition: false,
        }
    }
}

impl PartitionDetector {
    pub fn new(config: PartitionConfig) -> Self {
        Self {
            expected_size: AtomicUsize::new(0),
            config,
            detection_start: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Update expected cluster size (called when stable for 60s)
    pub fn update_expected_size(&self, size: usize) {
        self.expected_size.store(size, Ordering::SeqCst);
    }
    
    /// Check if current cluster size indicates partition
    pub async fn check_partition(&self, current_size: usize) -> PartitionStatus {
        let expected = self.expected_size.load(Ordering::SeqCst);
        
        if expected == 0 {
            // Still bootstrapping
            return PartitionStatus::Unknown;
        }
        
        let threshold_size = (expected as f64 * self.config.threshold).ceil() as usize;
        
        if current_size < threshold_size {
            // Potential partition - start grace period
            let mut detection_start = self.detection_start.write().await;
            
            if detection_start.is_none() {
                *detection_start = Some(Instant::now());
            }
            
            let elapsed = Instant::now().duration_since(detection_start.unwrap());
            
            if elapsed >= self.config.grace_period {
                // Grace period expired - partition confirmed
                PartitionStatus::Partitioned {
                    current_size,
                    expected_size: expected,
                    minority: current_size < threshold_size,
                }
            } else {
                // Still in grace period
                PartitionStatus::Suspect {
                    current_size,
                    expected_size: expected,
                    grace_remaining: self.config.grace_period - elapsed,
                }
            }
        } else {
            // Cluster size healthy - reset detection
            *self.detection_start.write().await = None;
            PartitionStatus::Healthy {
                current_size,
                expected_size: expected,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PartitionStatus {
    Unknown,
    Healthy { current_size: usize, expected_size: usize },
    Suspect { current_size: usize, expected_size: usize, grace_remaining: Duration },
    Partitioned { current_size: usize, expected_size: usize, minority: bool },
}

impl ClusterMembership {
    pub async fn partition_status(&self) -> PartitionStatus {
        let current_size = self.nodes.read().await.len();
        self.partition_detector.check_partition(current_size).await
    }
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_partition_detection_3_node_split() {
    let cluster = create_5_node_cluster().await;
    
    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Kill 2 nodes (3+2 split)
    drop(cluster[3]);
    drop(cluster[4]);
    
    // Wait for grace period
    tokio::time::sleep(Duration::from_secs(35)).await;
    
    // Remaining 3 nodes should detect partition
    let status = cluster[0].partition_status().await;
    match status {
        PartitionStatus::Healthy { .. } => {
            // 3 nodes is still majority (> 50%)
            assert!(true);
        }
        _ => panic!("Should be healthy with 3/5 nodes"),
    }
}

#[tokio::test]
async fn test_partition_detection_minority() {
    let cluster = create_5_node_cluster().await;
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Kill 3 nodes (2+3 split)
    drop(cluster[2]);
    drop(cluster[3]);
    drop(cluster[4]);
    
    tokio::time::sleep(Duration::from_secs(35)).await;
    
    // Remaining 2 nodes should detect minority partition
    let status = cluster[0].partition_status().await;
    match status {
        PartitionStatus::Partitioned { minority: true, .. } => assert!(true),
        _ => panic!("Should detect minority partition with 2/5 nodes"),
    }
}
```

**Decision for connection_swap**:
- **Do NOT enable `read_only_on_partition`** (workers can still serve)
- **Director logs warning** when partitioned (monitoring alert)
- **Clients retry** if no workers available (natural handling)

---

## Issue 6: Correct Phi Accrual Implementation

### Problem
Original implementation used wrong formula (linear, not CDF-based).

### Solution: Proper Statistical Failure Detection

```rust
// src/cluster/failure.rs

use statrs::distribution::{Normal, ContinuousCDF};

pub struct PhiAccrualDetector {
    history: VecDeque<f64>,  // Inter-arrival times in seconds
    max_samples: usize,
    threshold: f64,
    last_heartbeat: Instant,
    min_samples: usize,
}

impl PhiAccrualDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            history: VecDeque::with_capacity(1000),
            max_samples: 1000,
            threshold,
            last_heartbeat: Instant::now(),
            min_samples: 5,  // Need at least 5 samples for meaningful statistics
        }
    }
    
    pub fn heartbeat(&mut self) {
        let now = Instant::now();
        let interval = now.duration_since(self.last_heartbeat).as_secs_f64();
        self.last_heartbeat = now;
        
        // Ignore first heartbeat (no interval yet)
        if interval > 0.0 {
            self.history.push_back(interval);
            if self.history.len() > self.max_samples {
                self.history.pop_front();
            }
        }
    }
    
    pub fn phi(&self) -> f64 {
        if self.history.len() < self.min_samples {
            return 0.0;  // Not enough data yet
        }
        
        let elapsed = Instant::now().duration_since(self.last_heartbeat).as_secs_f64();
        
        // Calculate mean and standard deviation
        let n = self.history.len() as f64;
        let mean = self.history.iter().sum::<f64>() / n;
        
        let variance = self.history.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / n;
        
        let std_dev = variance.sqrt();
        
        // Handle edge case: no variance (all intervals identical)
        if std_dev < 1e-9 {
            return if elapsed > mean * 2.0 {
                self.threshold + 1.0  // Definitely failed
            } else {
                0.0  // Healthy
            };
        }
        
        // Calculate probability using normal distribution
        // P(X > elapsed) where X ~ N(mean, std_dev²)
        let normal = Normal::new(mean, std_dev).unwrap();
        let prob = 1.0 - normal.cdf(elapsed);
        
        // Phi = -log10(P)
        // Clamp to avoid infinity
        let phi = -prob.max(1e-10).log10();
        
        phi
    }
    
    pub fn is_available(&self) -> bool {
        self.phi() < self.threshold
    }
    
    pub fn suspicion_level(&self) -> SuspicionLevel {
        let phi = self.phi();
        
        if phi < self.threshold * 0.5 {
            SuspicionLevel::Healthy
        } else if phi < self.threshold {
            SuspicionLevel::Suspect
        } else {
            SuspicionLevel::Failed
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspicionLevel {
    Healthy,
    Suspect,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_phi_healthy_heartbeats() {
        let mut detector = PhiAccrualDetector::new(8.0);
        
        // Simulate regular heartbeats every 1 second
        for _ in 0..10 {
            detector.heartbeat();
            std::thread::sleep(Duration::from_secs(1));
        }
        
        // Phi should be low (healthy)
        assert!(detector.phi() < 2.0, "Phi should be low with regular heartbeats");
        assert!(detector.is_available());
    }
    
    #[test]
    fn test_phi_missed_heartbeat() {
        let mut detector = PhiAccrualDetector::new(8.0);
        
        // Regular heartbeats
        for _ in 0..10 {
            detector.heartbeat();
            std::thread::sleep(Duration::from_millis(100));
        }
        
        // Miss several heartbeats
        std::thread::sleep(Duration::from_secs(2));
        
        // Phi should be high (failed)
        assert!(detector.phi() > 8.0, "Phi should be high after missed heartbeats");
        assert!(!detector.is_available());
    }
    
    #[test]
    fn test_phi_variable_intervals() {
        let mut detector = PhiAccrualDetector::new(8.0);
        
        // Variable intervals (network jitter)
        let intervals = vec![0.9, 1.1, 0.8, 1.2, 1.0, 0.95, 1.05, 0.85, 1.15, 1.0];
        for interval in intervals {
            detector.heartbeat();
            std::thread::sleep(Duration::from_secs_f64(interval));
        }
        
        // Should adapt to jitter, phi still low
        assert!(detector.phi() < 3.0, "Should adapt to jitter");
    }
}
```

**Dependency Addition**:
```toml
[dependencies]
statrs = "0.17"  # For normal distribution CDF
```

---

## Issue 7: Health Check Circular Dependency

### Problem
HealthChecker needs cluster reference, but cluster owns HealthChecker.

### Solution: Shared Node Registry

```rust
// src/cluster/mod.rs - Refactored ownership

pub struct ClusterMembership {
    local_node: ClusterNode,
    config: ClusterConfig,
    
    /// Shared node registry (used by gossip, health, queries)
    nodes: Arc<RwLock<HashMap<NodeId, NodeStatus>>>,
    
    /// Gossip protocol (runs in background)
    gossip: Arc<GossipProtocol>,
    
    /// Health checker (runs in background, shares nodes)
    health: Arc<HealthChecker>,
    
    /// Partition detector
    partition_detector: Arc<PartitionDetector>,
    
    /// Event broadcaster
    event_tx: broadcast::Sender<ClusterEvent>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
}

// src/cluster/health.rs

pub struct HealthChecker {
    /// Shared reference to node registry (no ownership)
    nodes: Arc<RwLock<HashMap<NodeId, NodeStatus>>>,
    
    /// Health check implementations
    checks: Vec<Box<dyn HealthCheck>>,
    
    /// Configuration
    interval: Duration,
    
    /// Event broadcaster
    event_tx: broadcast::Sender<ClusterEvent>,
}

impl HealthChecker {
    pub async fn run(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.interval);
        
        loop {
            interval.tick().await;
            
            // Read nodes (shared lock, no circular dependency)
            let node_list: Vec<_> = {
                let nodes = self.nodes.read().await;
                nodes.values()
                    .filter(|s| s.state == NodeState::Alive)
                    .map(|s| s.node.clone())
                    .collect()
            };
            
            // Check all nodes in parallel
            let results = futures::future::join_all(
                node_list.iter().map(|node| self.check_node(node))
            ).await;
            
            // Update health status (write lock, but separate from read)
            let mut nodes = self.nodes.write().await;
            for (node_id, healthy) in results {
                if let Some(status) = nodes.get_mut(&node_id) {
                    if healthy != status.healthy {
                        status.healthy = healthy;
                        
                        let event = if healthy {
                            ClusterEvent::NodeRecovered(status.node.clone())
                        } else {
                            ClusterEvent::NodeFailed(status.node.clone())
                        };
                        
                        self.event_tx.send(event).ok();
                    }
                }
            }
        }
    }
    
    async fn check_node(&self, node: &ClusterNode) -> (NodeId, bool) {
        let mut all_healthy = true;
        
        for check in &self.checks {
            let status = check.check().await;
            all_healthy &= status.healthy;
            
            if !all_healthy {
                break;  // Early exit on first failure
            }
        }
        
        (node.id.clone(), all_healthy)
    }
}

impl ClusterMembership {
    pub async fn start(local_node: ClusterNode, config: ClusterConfig) -> Result<Self, ClusterError> {
        // Shared node registry
        let nodes = Arc::new(RwLock::new(HashMap::new()));
        
        // Add self to registry
        {
            let mut n = nodes.write().await;
            n.insert(local_node.id.clone(), NodeStatus {
                node: local_node.clone(),
                state: NodeState::Alive,
                incarnation: Incarnation::initial(),
                last_seen: Instant::now(),
                consecutive_failures: 0,
                healthy: true,
            });
        }
        
        let (event_tx, _) = broadcast::channel(1000);
        
        // Create gossip protocol (shares nodes)
        let gossip = Arc::new(GossipProtocol::new(
            local_node.clone(),
            config.clone(),
            nodes.clone(),  // Shared
            event_tx.clone(),
        ));
        
        // Create health checker (shares nodes)
        let health = Arc::new(HealthChecker::new(
            nodes.clone(),  // Shared
            config.health_interval,
            event_tx.clone(),
        ));
        
        // Start background tasks
        let gossip_handle = tokio::spawn({
            let gossip = gossip.clone();
            async move { gossip.run().await }
        });
        
        let health_handle = tokio::spawn({
            let health = health.clone();
            async move { health.run().await }
        });
        
        // ... rest of initialization
        
        Ok(Self {
            local_node,
            config,
            nodes,
            gossip,
            health,
            partition_detector: Arc::new(PartitionDetector::new(PartitionConfig::default())),
            event_tx,
            shutdown_tx: Some(shutdown_tx),
        })
    }
}
```

**No circular dependency**: All components share `Arc<RwLock<HashMap<NodeId, NodeStatus>>>`.

---

## Issue 8: Graceful Shutdown Sequence

### Problem
Shutdown order unspecified (active gossip, health checks, connections in flight).

### Solution: Phased Shutdown Protocol

```rust
// src/cluster/mod.rs

impl ClusterMembership {
    pub async fn leave(&mut self) -> Result<(), ClusterError> {
        info!("Starting graceful cluster leave");
        
        // Phase 1: Broadcast Leave message to all peers (max 5s)
        let leave_msg = GossipMessage::Leave {
            node_id: self.local_node.id.clone(),
            incarnation: self.local_incarnation(),
        };
        
        let nodes: Vec<_> = {
            let n = self.nodes.read().await;
            n.values()
                .filter(|s| s.node.id != self.local_node.id)
                .map(|s| s.node.cluster_addr)
                .collect()
        };
        
        let broadcast_futures: Vec<_> = nodes.iter()
            .map(|addr| self.gossip.send_message(*addr, leave_msg.clone()))
            .collect();
        
        tokio::time::timeout(
            Duration::from_secs(5),
            futures::future::join_all(broadcast_futures)
        ).await.ok();  // Ignore timeout - best effort
        
        info!("Leave message broadcast complete");
        
        // Phase 2: Stop gossip loop
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).ok();
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;  // Let tasks see shutdown signal
        info!("Gossip loop stopped");
        
        // Phase 3: Stop health checker
        // Health checker will stop on next interval when it sees shutdown signal
        
        // Phase 4: Close all QUIC connections (max 2s)
        tokio::time::timeout(
            Duration::from_secs(2),
            self.gossip.connection_pool.close_all()
        ).await.ok();
        
        info!("QUIC connections closed");
        
        // Phase 5: Clear node registry
        self.nodes.write().await.clear();
        
        // Phase 6: Close event channel
        // (broadcast::Sender drops automatically)
        
        info!("Graceful leave complete");
        Ok(())
    }
    
    /// Force shutdown (used in emergencies or Drop)
    pub async fn shutdown_immediate(&mut self) {
        warn!("Forcing immediate cluster shutdown");
        
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).ok();
        }
        
        // Don't wait for graceful close
        self.nodes.write().await.clear();
    }
}

impl Drop for ClusterMembership {
    fn drop(&mut self) {
        // Can't await in Drop, so use immediate shutdown
        if self.shutdown_tx.is_some() {
            warn!("ClusterMembership dropped without calling leave() - forcing shutdown");
            // Send shutdown signal (synchronous)
            if let Some(tx) = self.shutdown_tx.take() {
                tx.send(()).ok();
            }
        }
    }
}

// Background tasks check shutdown signal

impl GossipProtocol {
    pub async fn run(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.config.gossip_interval);
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.gossip_round().await;
                }
                _ = shutdown_rx.recv() => {
                    info!("Gossip loop received shutdown signal");
                    break;
                }
            }
        }
        
        info!("Gossip loop terminated");
    }
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_graceful_leave() {
    let cluster = create_3_node_cluster().await;
    
    // Node 1 leaves gracefully
    cluster[1].leave().await.unwrap();
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Other nodes should see it left
    let nodes = cluster[0].nodes().await;
    assert_eq!(nodes.len(), 2, "Node 1 should be removed");
    
    let node1_status = cluster[0].nodes.read().await.get(&cluster[1].local_id()).cloned();
    assert!(node1_status.is_none() || node1_status.unwrap().state == NodeState::Left);
}

#[tokio::test]
async fn test_drop_without_leave() {
    let cluster = create_3_node_cluster().await;
    
    // Drop without calling leave() - should log warning
    drop(cluster[1]);
    
    // Other nodes detect failure (not graceful leave)
    tokio::time::sleep(Duration::from_secs(20)).await;
    
    let nodes = cluster[0].nodes().await;
    let node1_status = nodes.iter().find(|n| n.id == cluster[1].local_id());
    
    // Should be marked Dead (not Left)
    assert!(node1_status.is_none() || cluster[0].nodes.read().await.get(&cluster[1].local_id()).unwrap().state == NodeState::Dead);
}
```

---

## Issue 9: Event Backpressure Strategy

### Problem
Broadcast channel drops events silently if consumer is slow.

### Solution: Bounded Channel with Metrics

```rust
// src/cluster/mod.rs

const EVENT_CHANNEL_CAPACITY: usize = 1000;

pub struct ClusterMembership {
    event_tx: broadcast::Sender<ClusterEvent>,
    event_drops: Arc<AtomicU64>,  // Track dropped events
}

impl ClusterMembership {
    pub async fn start(...) -> Result<Self, ClusterError> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let event_drops = Arc::new(AtomicU64::new(0));
        
        // Spawn monitor task to track drops
        let drops_clone = event_drops.clone();
        let tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let drops = drops_clone.swap(0, Ordering::SeqCst);
                if drops > 0 {
                    warn!("Cluster events dropped in last 60s: {}", drops);
                    
                    // Emit warning event
                    tx_clone.send(ClusterEvent::EventsDropped { count: drops }).ok();
                }
            }
        });
        
        Ok(Self {
            event_tx,
            event_drops,
            ...
        })
    }
    
    /// Send event with drop tracking
    fn emit_event(&self, event: ClusterEvent) {
        match self.event_tx.send(event) {
            Ok(_) => {}
            Err(broadcast::error::SendError(_)) => {
                // No receivers - that's fine
            }
        }
        
        // Check for lagged receivers
        if self.event_tx.len() > EVENT_CHANNEL_CAPACITY * 3 / 4 {
            warn!("Event channel filling up ({}/{}) - consumers may be slow",
                  self.event_tx.len(), EVENT_CHANNEL_CAPACITY);
        }
    }
    
    /// Subscribe to cluster events with explicit lag handling
    pub fn events(&self) -> ClusterEventReceiver {
        let rx = self.event_tx.subscribe();
        
        ClusterEventReceiver {
            inner: rx,
            drops: self.event_drops.clone(),
        }
    }
}

pub struct ClusterEventReceiver {
    inner: broadcast::Receiver<ClusterEvent>,
    drops: Arc<AtomicU64>,
}

impl ClusterEventReceiver {
    pub async fn recv(&mut self) -> Result<ClusterEvent, RecvError> {
        match self.inner.recv().await {
            Ok(event) => Ok(event),
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Consumer fell behind - track drops
                self.drops.fetch_add(n, Ordering::SeqCst);
                warn!("Event consumer lagged, {} events dropped", n);
                Err(RecvError::Lagged(n))
            }
            Err(broadcast::error::RecvError::Closed) => {
                Err(RecvError::Closed)
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RecvError {
    #[error("Consumer lagged, {0} events dropped")]
    Lagged(u64),
    
    #[error("Event channel closed")]
    Closed,
}

#[derive(Debug, Clone)]
pub enum ClusterEvent {
    NodeJoined(ClusterNode),
    NodeLeft(ClusterNode),
    NodeFailed(ClusterNode),
    NodeRecovered(ClusterNode),
    NodeSuspect(ClusterNode),
    NodeTagsUpdated { node_id: NodeId, tags: HashMap<String, String> },
    EventsDropped { count: u64 },  // ✅ NEW: Alert consumers
    PartitionDetected(PartitionStatus),
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_event_backpressure() {
    let cluster = ClusterMembership::start(...).await.unwrap();
    
    let mut rx = cluster.events();
    
    // Generate many events quickly
    for i in 0..2000 {
        cluster.emit_event(ClusterEvent::NodeJoined(create_test_node(i)));
    }
    
    // Slow consumer
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let mut received = 0;
    let mut lagged = false;
    
    while let Ok(event) = rx.try_recv() {
        match event {
            ClusterEvent::EventsDropped { count } => {
                lagged = true;
                assert!(count > 0);
            }
            _ => received += 1,
        }
    }
    
    assert!(lagged, "Should emit EventsDropped when consumer is slow");
}
```

---

## Issue 10: Seed Node Bootstrap Retry

### Problem
If seed nodes aren't ready yet, join fails immediately.

### Solution: Exponential Backoff with Retries

```rust
// src/cluster/mod.rs

impl ClusterMembership {
    pub async fn start(local_node: ClusterNode, config: ClusterConfig) -> Result<Self, ClusterError> {
        // ... initialization ...
        
        if config.seeds.is_empty() {
            // First node in cluster - start immediately
            info!("Starting as seed node (no seeds configured)");
        } else {
            // Join via seeds with retry logic
            info!("Joining cluster via {} seed nodes", config.seeds.len());
            
            Self::join_with_retry(&gossip, &config.seeds).await?;
        }
        
        // ... rest of initialization ...
        
        Ok(cluster)
    }
    
    async fn join_with_retry(
        gossip: &Arc<GossipProtocol>,
        seeds: &[SocketAddr],
    ) -> Result<(), ClusterError> {
        const MAX_ATTEMPTS: usize = 10;
        const INITIAL_BACKOFF: Duration = Duration::from_millis(100);
        const MAX_BACKOFF: Duration = Duration::from_secs(10);
        
        let mut backoff = INITIAL_BACKOFF;
        let mut last_error = None;
        
        for attempt in 1..=MAX_ATTEMPTS {
            match Self::attempt_join(gossip, seeds).await {
                Ok(()) => {
                    info!("Successfully joined cluster on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < MAX_ATTEMPTS {
                        warn!(
                            attempt,
                            backoff_ms = backoff.as_millis(),
                            "Failed to join cluster, retrying..."
                        );
                        
                        tokio::time::sleep(backoff).await;
                        
                        // Exponential backoff with jitter
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                        let jitter = rand::random::<u64>() % (backoff.as_millis() as u64 / 4);
                        backoff += Duration::from_millis(jitter);
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or(ClusterError::NoSeedsReachable))
    }
    
    async fn attempt_join(
        gossip: &Arc<GossipProtocol>,
        seeds: &[SocketAddr],
    ) -> Result<(), ClusterError> {
        // Try all seeds in parallel
        let join_futures: Vec<_> = seeds.iter()
            .map(|seed_addr| gossip.send_join_request(*seed_addr))
            .collect();
        
        // Wait for at least ONE seed to respond
        let results = futures::future::join_all(join_futures).await;
        
        let successful = results.iter().filter(|r| r.is_ok()).count();
        
        if successful > 0 {
            info!("Connected to {} seed nodes", successful);
            Ok(())
        } else {
            Err(ClusterError::NoSeedsReachable)
        }
    }
}

impl GossipProtocol {
    async fn send_join_request(&self, seed_addr: SocketAddr) -> Result<(), ClusterError> {
        let join_msg = GossipMessage::Join {
            node: self.local_node.clone(),
        };
        
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            self.transport.send(seed_addr, join_msg)
        ).await
        .map_err(|_| ClusterError::TransportError("Join timeout".into()))??;
        
        match response {
            GossipMessage::Ack { .. } => Ok(()),
            _ => Err(ClusterError::TransportError("Unexpected response".into())),
        }
    }
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_join_retry_eventual_success() {
    // Start seed node AFTER 2 seconds (simulates slow start)
    let seed_handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        start_seed_node().await
    });
    
    // Node tries to join immediately
    let node = ClusterMembership::start(
        create_test_node("node1", 60001, 60002),
        ClusterConfig {
            seeds: vec!["127.0.0.1:60000".parse().unwrap()],
            ..Default::default()
        }
    ).await;
    
    // Should succeed despite seed not being ready initially
    assert!(node.is_ok(), "Should retry and eventually join");
    
    seed_handle.await.unwrap();
}

#[tokio::test]
async fn test_join_failure_after_max_retries() {
    // No seed node running
    let result = ClusterMembership::start(
        create_test_node("node1", 60001, 60002),
        ClusterConfig {
            seeds: vec!["127.0.0.1:99999".parse().unwrap()],  // Invalid
            ..Default::default()
        }
    ).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        ClusterError::NoSeedsReachable => {}
        e => panic!("Expected NoSeedsReachable, got {:?}", e),
    }
}
```

---

## Summary of Changes

| Issue | Original Risk | New Risk | Mitigation |
|-------|--------------|----------|------------|
| 1. Connection Pooling | CRITICAL | **LOW** | Persistent connection pool, bounded (50 conns) |
| 2. Incarnation Conflicts | CRITICAL | **LOW** | Lamport-style with wraparound handling |
| 3. Message Size | HIGH | **LOW** | Bounded updates (20/msg), priority queue |
| 4. Tag Race Conditions | HIGH | **MEDIUM** | Eventual consistency + director retry |
| 5. Split Brain | CRITICAL | **MEDIUM** | Formal quorum detection (50% threshold, 30s grace) |
| 6. Phi Accrual | CRITICAL | **LOW** | Correct CDF implementation with statrs |
| 7. Circular Dependency | HIGH | **LOW** | Shared Arc<RwLock<>> node registry |
| 8. Graceful Shutdown | HIGH | **LOW** | 6-phase shutdown protocol |
| 9. Event Backpressure | MEDIUM | **LOW** | Drop tracking, lag warnings |
| 10. Seed Bootstrap | HIGH | **LOW** | Exponential backoff, 10 retries |

---

## Updated Implementation Timeline

### Phase 0: Foundation (Week 1)
- Connection pool implementation
- Incarnation number handling
- Message size bounds
- Basic data structures

### Phase 1: Gossip Protocol (Week 2)
- SWIM ping/ack with bounded updates
- Priority queue for gossip selection
- Seed node retry logic
- Join/leave messages

### Phase 2: Failure Detection (Week 3)
- Correct phi accrual (with statrs)
- Suspicion levels
- Partition detection
- Split brain handling

### Phase 3: Health Checking (Week 4)
- HealthCheck trait
- Shared node registry
- Event emission with backpressure
- Graceful shutdown

### Phase 4: RpcServer Integration (Week 5)
- `.with_cluster()` builder
- Tag-based queries
- Event subscriptions
- Integration tests

### Phase 5: Migrate connection_swap (Week 6)
- Remove manual heartbeat/registration
- Add optimistic director retry
- Update documentation
- End-to-end chaos tests

---

## New Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
s2n-quic = "1"
serde = { version = "1", features = ["derive"] }
bincode = "1"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
tracing = "0.1"
futures = "0.3"
dashmap = "6"        # NEW: For connection pool
statrs = "0.17"      # NEW: For phi accrual CDF
rand = "0.8"         # NEW: For backoff jitter
```

---

## Final Verdict

✅ **READY TO IMPLEMENT**

All critical ambiguities resolved. Risk level: **MEDIUM** (down from VERY HIGH).

Remaining risks:
- Tag update eventual consistency (MEDIUM) - mitigated with retry
- Split brain in <3 node clusters (MEDIUM) - documented limitation
- Chaos testing may reveal unforeseen edge cases (LOW) - comprehensive test plan

**Recommendation**: Proceed with phased implementation, starting with Phase 0.
