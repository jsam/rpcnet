# RpcNet Cluster Management Design

## Executive Summary

Add opt-in cluster management to rpcnet that provides:
- Automatic peer discovery via gossip
- Built-in failure detection
- Health checking infrastructure  
- Service registry with tag-based queries

**Success Criteria**: connection_swap example works without manual heartbeat/registration code.

---

## Current State Analysis

### What connection_swap Does Manually (Should Be Framework)

```rust
// examples/connection_swap/src/bin/worker.rs
loop {
    let available = *is_available.read().await;
    let register_req = RegisterWorkerRequest { worker: WorkerInfo { ... } };
    director_client.call("register_worker", req_bytes).await?;
    sleep(Duration::from_secs(5)).await;  // Manual heartbeat
}

// examples/connection_swap/src/bin/director.rs
server.register("register_worker", move |params: Vec<u8>| {
    // Manual registry management
    workers.write().await.insert(worker_id, worker_info);
});

// examples/connection_swap/src/bin/client.rs
loop {
    heartbeat_interval.tick().await;
    worker_client.call("heartbeat", req_bytes).await?;  // Manual failure detection
    if consecutive_failures >= 2 {
        heartbeat_failed.store(true, Ordering::SeqCst);
        break;
    }
}
```

**Problems**:
1. Every app reimplements the same patterns
2. Hard to test (timing-dependent)
3. Edge cases not handled (split brain, network partitions)
4. No peer discovery (must manually configure director address)

### What Should Be Framework Behavior

```rust
// Worker - zero management code
let server = RpcServer::new(user_config)
    .with_cluster(ClusterConfig {
        cluster_addr: "127.0.0.1:63001",
        seeds: vec!["127.0.0.1:61001"],  // Director's cluster port
        tags: hashmap!{ "role" => "worker", "capacity" => "10" },
    })?;

server.register_streaming("generate", handler).await;
server.start().await?;
// Framework automatically: joins cluster, sends heartbeats, health checks

// Director - query cluster state
let workers = server.cluster()
    .nodes_with_tag("role", "worker")
    .filter(|n| server.cluster().is_healthy(&n.id))
    .collect::<Vec<_>>();

let chosen_worker = workers[idx % workers.len()];

// Client - failure detection via cluster
// (keep current direct heartbeat for demo purposes, but could use cluster)
```

---

## Design Principles

1. **Opt-in, not mandatory** - Simple apps don't need clustering
2. **Zero breaking changes** - Existing code continues to work
3. **Separate concerns** - Cluster port separate from RPC port
4. **Observable** - Cluster events exposed via stream
5. **Testable** - Deterministic unit tests, not just integration tests

---

## Architecture

### Module Structure

```
src/
├── lib.rs              (existing RpcServer/RpcClient)
├── cluster/
│   ├── mod.rs          (ClusterMembership public API)
│   ├── config.rs       (ClusterConfig)
│   ├── node.rs         (ClusterNode, NodeState)
│   ├── gossip.rs       (SWIM gossip protocol)
│   ├── health.rs       (Health check scheduling)
│   ├── failure.rs      (Failure detector - phi accrual)
│   ├── registry.rs     (Tag-based node registry)
│   └── transport.rs    (Cluster message transport over QUIC)
```

### Data Model

```rust
// src/cluster/node.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: NodeId,
    pub rpc_addr: SocketAddr,      // User traffic (e.g., 62001)
    pub cluster_addr: SocketAddr,  // Cluster management (e.g., 63001)
    pub tags: HashMap<String, String>,
    pub metadata: NodeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub version: String,
    pub started_at: SystemTime,
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Alive,      // Responding to health checks
    Suspect,    // Failed health check, investigating
    Dead,       // Confirmed dead, will be removed
    Left,       // Gracefully left cluster
}

// Internal state tracking
struct NodeStatus {
    node: ClusterNode,
    state: NodeState,
    incarnation: u64,              // SWIM incarnation number
    last_seen: Instant,
    consecutive_failures: u32,
}
```

### Configuration

```rust
// src/cluster/config.rs
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Address for cluster management traffic (gossip, health checks)
    pub cluster_addr: SocketAddr,
    
    /// Seed nodes to join (at least one must be reachable)
    pub seeds: Vec<SocketAddr>,
    
    /// Tags for service discovery (e.g., role=worker, dc=us-east)
    pub tags: HashMap<String, String>,
    
    /// Gossip interval (default: 1s)
    pub gossip_interval: Duration,
    
    /// Health check interval (default: 5s)
    pub health_interval: Duration,
    
    /// Suspicion timeout before marking node dead (default: 10s)
    pub suspicion_timeout: Duration,
    
    /// Number of random nodes to gossip with per round (default: 3)
    pub gossip_fanout: usize,
    
    /// How long to keep dead nodes before removal (default: 60s)
    pub dead_node_ttl: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_addr: "0.0.0.0:0".parse().unwrap(),  // Random port
            seeds: vec![],
            tags: HashMap::new(),
            gossip_interval: Duration::from_secs(1),
            health_interval: Duration::from_secs(5),
            suspicion_timeout: Duration::from_secs(10),
            gossip_fanout: 3,
            dead_node_ttl: Duration::from_secs(60),
        }
    }
}
```

### Public API

```rust
// src/cluster/mod.rs
pub struct ClusterMembership {
    local_node: ClusterNode,
    config: ClusterConfig,
    nodes: Arc<RwLock<HashMap<NodeId, NodeStatus>>>,
    event_tx: broadcast::Sender<ClusterEvent>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ClusterMembership {
    /// Start cluster membership (called by RpcServer)
    pub async fn start(
        local_node: ClusterNode,
        config: ClusterConfig,
    ) -> Result<Self, ClusterError>;
    
    /// Get all nodes in cluster
    pub async fn nodes(&self) -> Vec<ClusterNode>;
    
    /// Get nodes with specific tag
    pub async fn nodes_with_tag(&self, key: &str, value: &str) -> Vec<ClusterNode>;
    
    /// Get nodes in specific state
    pub async fn nodes_in_state(&self, state: NodeState) -> Vec<ClusterNode>;
    
    /// Check if node is healthy
    pub async fn is_healthy(&self, node_id: &NodeId) -> bool;
    
    /// Get local node ID
    pub fn local_id(&self) -> &NodeId;
    
    /// Get local node info
    pub fn local_node(&self) -> &ClusterNode;
    
    /// Subscribe to cluster events
    pub fn events(&self) -> broadcast::Receiver<ClusterEvent>;
    
    /// Leave cluster gracefully
    pub async fn leave(&mut self) -> Result<(), ClusterError>;
}

#[derive(Debug, Clone)]
pub enum ClusterEvent {
    NodeJoined(ClusterNode),
    NodeLeft(ClusterNode),
    NodeFailed(ClusterNode),
    NodeRecovered(ClusterNode),
    NodeSuspect(ClusterNode),
}

#[derive(Debug, Error)]
pub enum ClusterError {
    #[error("Failed to bind cluster address: {0}")]
    BindError(String),
    
    #[error("No seed nodes reachable")]
    NoSeedsReachable,
    
    #[error("Transport error: {0}")]
    TransportError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    
    #[error("Already in cluster")]
    AlreadyJoined,
}
```

### Integration with RpcServer

```rust
// src/lib.rs - additions to RpcServer
pub struct RpcServer {
    // ... existing fields ...
    cluster: Option<Arc<ClusterMembership>>,
}

impl RpcServer {
    /// Enable cluster mode
    pub fn with_cluster(mut self, config: ClusterConfig) -> Result<Self, RpcError> {
        // Validation
        if config.seeds.is_empty() {
            return Err(RpcError::ConfigurationError(
                "Cluster requires at least one seed node".into()
            ));
        }
        
        if config.cluster_addr.port() == 0 {
            return Err(RpcError::ConfigurationError(
                "Cluster address must have explicit port".into()
            ));
        }
        
        // Create local node descriptor
        let local_node = ClusterNode {
            id: NodeId(Uuid::new_v4()),
            rpc_addr: self.socket_addr.ok_or_else(|| 
                RpcError::ConfigurationError("RPC address not set".into()))?,
            cluster_addr: config.cluster_addr,
            tags: config.tags.clone(),
            metadata: NodeMetadata {
                version: env!("CARGO_PKG_VERSION").to_string(),
                started_at: SystemTime::now(),
                custom: HashMap::new(),
            },
        };
        
        self.cluster_config = Some((local_node, config));
        Ok(self)
    }
    
    /// Access cluster membership (None if clustering not enabled)
    pub fn cluster(&self) -> Option<&ClusterMembership> {
        self.cluster.as_ref().map(|c| c.as_ref())
    }
    
    /// Start server (modified to start cluster if configured)
    pub async fn start(&mut self, quic: QuicConnection) -> Result<(), RpcError> {
        // Start cluster membership if configured
        if let Some((local_node, config)) = self.cluster_config.take() {
            let cluster = ClusterMembership::start(local_node, config).await
                .map_err(|e| RpcError::ClusterError(e.to_string()))?;
            self.cluster = Some(Arc::new(cluster));
        }
        
        // ... existing server start logic ...
    }
}
```

---

## Gossip Protocol (SWIM)

### Why SWIM?

- **Scalable**: O(log N) detection time, constant message load per node
- **Robust**: Handles transient failures, network partitions
- **Proven**: Used in Consul, Memberlist, Serf
- **Simple**: ~500 LOC for basic implementation

### SWIM Messages

```rust
// src/cluster/gossip.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
enum GossipMessage {
    /// Periodic gossip: "I think these nodes are alive/suspect/dead"
    Ping {
        from: NodeId,
        incarnation: u64,
        nodes: Vec<NodeUpdate>,
    },
    
    /// Response to ping: "I'm alive"
    Ack {
        from: NodeId,
        incarnation: u64,
    },
    
    /// Indirect ping: "Check if node X is alive"
    PingReq {
        from: NodeId,
        target: NodeId,
    },
    
    /// State updates: "Node X changed state"
    NodeUpdate {
        node_id: NodeId,
        state: NodeState,
        incarnation: u64,
    },
    
    /// Join request: "I want to join the cluster"
    Join {
        node: ClusterNode,
    },
    
    /// Leave notification: "I'm leaving gracefully"
    Leave {
        node_id: NodeId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeUpdate {
    node_id: NodeId,
    node: ClusterNode,
    state: NodeState,
    incarnation: u64,
}
```

### Gossip Algorithm

```rust
// Gossip round (runs every config.gossip_interval)
async fn gossip_round(&mut self) {
    // 1. Select random nodes to gossip with
    let targets = self.select_gossip_targets(self.config.gossip_fanout);
    
    // 2. For each target, send ping with state updates
    for target in targets {
        let updates = self.get_state_updates();
        let ping = GossipMessage::Ping {
            from: self.local_node.id.clone(),
            incarnation: self.incarnation,
            nodes: updates,
        };
        
        match self.send_gossip(&target.cluster_addr, ping).await {
            Ok(GossipMessage::Ack { .. }) => {
                // Node responded - mark as alive
                self.mark_node_alive(&target.id);
            }
            Err(_) => {
                // No response - initiate indirect ping
                self.indirect_ping(&target).await;
            }
        }
    }
    
    // 3. Process any received gossip
    while let Some(msg) = self.gossip_rx.try_recv() {
        self.handle_gossip_message(msg).await;
    }
    
    // 4. Check for suspected/dead nodes
    self.process_suspicions().await;
}

// Indirect ping (if direct ping fails)
async fn indirect_ping(&mut self, target: &ClusterNode) {
    // Ask K random nodes to ping target on our behalf
    let intermediaries = self.select_random_nodes(3);
    
    for intermediary in intermediaries {
        let ping_req = GossipMessage::PingReq {
            from: self.local_node.id.clone(),
            target: target.id.clone(),
        };
        
        if self.send_gossip(&intermediary.cluster_addr, ping_req).await.is_ok() {
            // Wait briefly for indirect ack
            if self.wait_for_indirect_ack(&target.id, Duration::from_millis(500)).await {
                // Target is alive (responded to intermediary)
                self.mark_node_alive(&target.id);
                return;
            }
        }
    }
    
    // All indirect pings failed - mark as suspect
    self.mark_node_suspect(&target.id);
}
```

---

## Health Checking

### Health Check Interface

```rust
// src/cluster/health.rs
#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: Option<String>,
    pub details: HashMap<String, String>,
}

// Built-in health checks
pub struct TcpHealthCheck {
    addr: SocketAddr,
    timeout: Duration,
}

pub struct HttpHealthCheck {
    url: String,
    timeout: Duration,
    expected_status: u16,
}

pub struct RpcHealthCheck {
    client: RpcClient,
    method: String,
    timeout: Duration,
}
```

### Health Check Scheduling

```rust
struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
    interval: Duration,
    results: Arc<RwLock<HashMap<NodeId, HealthStatus>>>,
}

impl HealthChecker {
    async fn run(&mut self) {
        let mut interval = tokio::time::interval(self.interval);
        
        loop {
            interval.tick().await;
            
            // Check all nodes in parallel
            let nodes = self.cluster.nodes().await;
            let futures: Vec<_> = nodes.iter()
                .map(|node| self.check_node(node))
                .collect();
            
            let results = futures::future::join_all(futures).await;
            
            // Update health status
            let mut status_map = self.results.write().await;
            for (node_id, status) in results {
                let previous = status_map.insert(node_id.clone(), status.clone());
                
                // Emit events on health changes
                if let Some(prev) = previous {
                    if prev.healthy != status.healthy {
                        if status.healthy {
                            self.event_tx.send(ClusterEvent::NodeRecovered(node)).ok();
                        } else {
                            self.event_tx.send(ClusterEvent::NodeFailed(node)).ok();
                        }
                    }
                }
            }
        }
    }
    
    async fn check_node(&self, node: &ClusterNode) -> (NodeId, HealthStatus) {
        // Run all health checks for this node
        let mut all_healthy = true;
        let mut details = HashMap::new();
        
        for check in &self.checks {
            let status = check.check().await;
            all_healthy &= status.healthy;
            details.extend(status.details);
        }
        
        (node.id.clone(), HealthStatus {
            healthy: all_healthy,
            message: None,
            details,
        })
    }
}
```

---

## Failure Detection (Phi Accrual)

### Why Phi Accrual?

- Adaptive to network conditions (unlike fixed timeouts)
- Produces continuous suspicion level (0.0 = alive, 1.0 = dead)
- Used in Cassandra, Akka Cluster

### Implementation

```rust
// src/cluster/failure.rs
struct PhiAccrualFailureDetector {
    history: VecDeque<Duration>,  // Inter-arrival times
    max_samples: usize,            // Window size (default: 1000)
    threshold: f64,                // Phi threshold (default: 8.0)
    last_heartbeat: Instant,
}

impl PhiAccrualFailureDetector {
    fn new(threshold: f64) -> Self {
        Self {
            history: VecDeque::with_capacity(1000),
            max_samples: 1000,
            threshold,
            last_heartbeat: Instant::now(),
        }
    }
    
    fn heartbeat(&mut self) {
        let now = Instant::now();
        let interval = now.duration_since(self.last_heartbeat);
        self.last_heartbeat = now;
        
        self.history.push_back(interval);
        if self.history.len() > self.max_samples {
            self.history.pop_front();
        }
    }
    
    fn phi(&self) -> f64 {
        if self.history.len() < 3 {
            return 0.0;  // Not enough data
        }
        
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_heartbeat);
        
        // Calculate mean and variance of inter-arrival times
        let mean = self.history.iter()
            .map(|d| d.as_secs_f64())
            .sum::<f64>() / self.history.len() as f64;
        
        let variance = self.history.iter()
            .map(|d| {
                let diff = d.as_secs_f64() - mean;
                diff * diff
            })
            .sum::<f64>() / self.history.len() as f64;
        
        let std_dev = variance.sqrt();
        
        // Phi = -log10(P(now - last_heartbeat))
        // Approximation using exponential distribution
        let phi = elapsed.as_secs_f64() / mean;
        -phi.log10()
    }
    
    fn is_available(&self) -> bool {
        self.phi() < self.threshold
    }
}
```

---

## Transport Layer

### Cluster Messages Over QUIC

```rust
// src/cluster/transport.rs
struct ClusterTransport {
    quic_server: s2n_quic::Server,
    quic_client: s2n_quic::Client,
    local_addr: SocketAddr,
}

impl ClusterTransport {
    async fn send(&self, target: SocketAddr, msg: GossipMessage) -> Result<GossipMessage, TransportError> {
        // Open QUIC connection
        let connect = Connect::new(target).with_server_name("cluster");
        let mut connection = self.quic_client.connect(connect).await?;
        
        // Open bidirectional stream
        let stream = connection.open_bidirectional_stream().await?;
        
        // Send message (length-prefixed)
        let bytes = bincode::serialize(&msg)?;
        let len = (bytes.len() as u32).to_le_bytes();
        stream.send_bytes(Bytes::from([&len[..], &bytes].concat())).await?;
        
        // Wait for response (with timeout)
        let response_bytes = tokio::time::timeout(
            Duration::from_secs(2),
            stream.receive_bytes()
        ).await??;
        
        let response = bincode::deserialize(&response_bytes)?;
        Ok(response)
    }
    
    async fn receive(&mut self) -> Result<(SocketAddr, GossipMessage), TransportError> {
        // Accept incoming connection
        let connection = self.quic_server.accept().await?;
        let remote_addr = connection.remote_addr()?;
        
        // Accept stream
        let stream = connection.accept_bidirectional_stream().await?;
        
        // Read message
        let bytes = stream.receive_bytes().await?;
        let msg = bincode::deserialize(&bytes)?;
        
        Ok((remote_addr, msg))
    }
}
```

---

## Migration Guide: connection_swap Example

### Before (Manual Management)

```rust
// worker.rs - REMOVE 50+ lines of registration logic
loop {
    let register_req = RegisterWorkerRequest { ... };
    director_client.call("register_worker", req_bytes).await?;
    sleep(Duration::from_secs(5)).await;
}

// director.rs - REMOVE registry management
server.register("register_worker", |params| { ... });
let workers = Arc::new(RwLock::new(HashMap::new()));

// client.rs - KEEP direct heartbeat (for demo), but could use cluster
```

### After (Framework-Managed)

```rust
// worker.rs - 3 lines
let server = RpcServer::new(user_config)
    .with_cluster(ClusterConfig {
        cluster_addr: mgmt_addr.parse()?,
        seeds: vec![director_mgmt_target.parse()?],
        tags: hashmap!{
            "role" => "worker",
            "label" => worker_label.clone(),
            "available" => available.to_string(),
        },
    })?;

// Update availability tag dynamically
if !available {
    server.cluster().unwrap().update_tag("available", "false").await;
}

// director.rs - query cluster
let workers = server.cluster().unwrap()
    .nodes_with_tag("role", "worker")
    .filter(|n| {
        server.cluster().unwrap().is_healthy(&n.id) &&
        n.tags.get("available") == Some(&"true".to_string())
    })
    .collect::<Vec<_>>();

let worker = &workers[next_idx % workers.len()];

// Return worker's RPC address (not cluster address!)
GetWorkerResponse {
    success: true,
    worker_addr: Some(worker.rpc_addr.to_string()),
    ...
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_node_join_leave() {
    let node1 = create_test_node("node1", 60001, 60002);
    let node2 = create_test_node("node2", 60003, 60004);
    
    let cluster1 = ClusterMembership::start(node1.clone(), ClusterConfig {
        cluster_addr: node1.cluster_addr,
        seeds: vec![],  // First node
        ..Default::default()
    }).await.unwrap();
    
    let cluster2 = ClusterMembership::start(node2.clone(), ClusterConfig {
        cluster_addr: node2.cluster_addr,
        seeds: vec![node1.cluster_addr],  // Join via node1
        ..Default::default()
    }).await.unwrap();
    
    // Wait for gossip
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Both should see each other
    assert_eq!(cluster1.nodes().await.len(), 2);
    assert_eq!(cluster2.nodes().await.len(), 2);
    
    // Node2 leaves
    cluster2.leave().await.unwrap();
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Node1 should see only itself
    assert_eq!(cluster1.nodes().await.len(), 1);
}

#[tokio::test]
async fn test_failure_detection() {
    let node1 = create_test_node("node1", 60001, 60002);
    let node2 = create_test_node("node2", 60003, 60004);
    
    let cluster1 = ClusterMembership::start(node1, ClusterConfig {
        health_interval: Duration::from_millis(100),
        suspicion_timeout: Duration::from_millis(500),
        ..Default::default()
    }).await.unwrap();
    
    let cluster2 = ClusterMembership::start(node2, ClusterConfig {
        seeds: vec![cluster1.local_node().cluster_addr],
        ..Default::default()
    }).await.unwrap();
    
    let node2_id = cluster2.local_id().clone();
    
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(cluster1.is_healthy(&node2_id).await);
    
    // Kill node2 (drop cluster2)
    drop(cluster2);
    
    // Wait for failure detection
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!cluster1.is_healthy(&node2_id).await);
}

#[tokio::test]
async fn test_tag_based_discovery() {
    let cluster = create_test_cluster(5).await;
    
    // Tag some nodes as workers
    cluster[0].update_tag("role", "worker").await;
    cluster[1].update_tag("role", "worker").await;
    cluster[2].update_tag("role", "director").await;
    
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    let workers = cluster[3].nodes_with_tag("role", "worker").await;
    assert_eq!(workers.len(), 2);
    
    let directors = cluster[3].nodes_with_tag("role", "director").await;
    assert_eq!(directors.len(), 1);
}
```

### Integration Test: connection_swap

```rust
#[tokio::test]
async fn test_connection_swap_with_cluster() {
    // Start director
    let director = start_director_with_cluster().await;
    
    // Start workers
    let worker_a = start_worker_with_cluster("worker-a", 62001, 63001).await;
    let worker_b = start_worker_with_cluster("worker-b", 62002, 63002).await;
    
    // Wait for cluster convergence
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Director should see 2 workers
    let workers = director.cluster().unwrap().nodes_with_tag("role", "worker").await;
    assert_eq!(workers.len(), 2);
    
    // Start client
    let mut client = start_client().await;
    
    // Client should get worker assignment
    let tokens = client.stream_tokens().await;
    assert!(tokens.len() > 0);
    
    // Kill worker-a
    drop(worker_a);
    
    // Wait for failure detection
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Director should see only 1 healthy worker
    let healthy_workers = director.cluster().unwrap()
        .nodes_with_tag("role", "worker")
        .filter(|n| director.cluster().unwrap().is_healthy(&n.id))
        .collect::<Vec<_>>();
    assert_eq!(healthy_workers.len(), 1);
    
    // Client should automatically reconnect to worker-b
    let more_tokens = client.stream_tokens().await;
    assert!(more_tokens.len() > 0);
}
```

---

## Implementation Plan

### Phase 1: Core Cluster (Week 1)
**Goal**: Basic cluster membership without health/failure detection

- [ ] Create `src/cluster/` module structure
- [ ] Implement `ClusterNode`, `NodeId`, `NodeState`
- [ ] Implement `ClusterConfig` with sensible defaults
- [ ] Implement basic `ClusterMembership` (just node storage)
- [ ] Write unit tests for data structures

**Success Criteria**: Can create ClusterMembership, no gossip yet

### Phase 2: Gossip Protocol (Week 2)
**Goal**: Nodes discover each other via SWIM gossip

- [ ] Implement `GossipMessage` types
- [ ] Implement `ClusterTransport` over QUIC
- [ ] Implement SWIM gossip rounds (ping/ack)
- [ ] Implement indirect pings (ping-req)
- [ ] Add join/leave messages
- [ ] Write unit tests for gossip protocol

**Success Criteria**: 
- Node A joins cluster via seed node B
- Node A sees Node B in `nodes()`
- Node C joins via Node A, sees both A and B
- Node A leaves gracefully, others stop seeing it

### Phase 3: Failure Detection (Week 3)
**Goal**: Detect and handle node failures

- [ ] Implement phi accrual failure detector
- [ ] Integrate with gossip (mark suspect → dead)
- [ ] Add suspicion timeout configuration
- [ ] Handle network partitions (split brain)
- [ ] Write unit tests for failure scenarios

**Success Criteria**:
- Kill node B, node A detects it within suspicion_timeout
- Node A marks Node B as Dead
- Node B removed from active nodes after dead_node_ttl

### Phase 4: Health Checking (Week 3-4)
**Goal**: Application-level health checks

- [ ] Implement `HealthCheck` trait
- [ ] Implement built-in health checks (TCP, RPC)
- [ ] Implement health check scheduler
- [ ] Emit ClusterEvents on health changes
- [ ] Write unit tests for health checks

**Success Criteria**:
- Register custom health check
- Health check runs every health_interval
- ClusterEvent::NodeFailed emitted when unhealthy

### Phase 5: RpcServer Integration (Week 4)
**Goal**: Make clustering opt-in for RpcServer

- [ ] Add `cluster: Option<Arc<ClusterMembership>>` to RpcServer
- [ ] Implement `with_cluster()` builder method
- [ ] Start cluster on `server.start()`
- [ ] Expose `cluster()` accessor
- [ ] Add cluster to RpcError variants
- [ ] Write integration tests

**Success Criteria**:
- Create RpcServer with `.with_cluster(config)`
- Server joins cluster on start
- Can query cluster state via `server.cluster()`

### Phase 6: Migrate connection_swap (Week 5)
**Goal**: Remove manual management from connection_swap

- [ ] Update worker.rs to use `.with_cluster()`
- [ ] Remove manual registration loop
- [ ] Update director.rs to query cluster
- [ ] Remove `register_worker` RPC endpoint
- [ ] Update protocol.rs (remove RegisterWorker types)
- [ ] Update README with new architecture
- [ ] Test full auto-healing scenario

**Success Criteria**:
- connection_swap works identically to before
- No manual heartbeat/registration code
- ~100 fewer LOC in example
- All auto-healing tests pass

### Phase 7: Documentation & Polish (Week 6)
- [ ] Write cluster module documentation
- [ ] Add cluster example (simple 3-node cluster)
- [ ] Document configuration options
- [ ] Add troubleshooting guide
- [ ] Benchmark cluster overhead
- [ ] Add CLUSTER_DESIGN.md to repo docs

---

## Edge Cases & Mitigations

### 1. Split Brain
**Problem**: Network partition creates two clusters

**Detection**:
- Gossip includes cluster size estimate
- If node sees cluster shrink significantly, suspect partition

**Mitigation**:
- Majority quorum (if < 50% of expected nodes, go read-only)
- External health check (ping known external service)
- Not solving fully (requires consensus like Raft/Paxos)

### 2. Thundering Herd on Rejoin
**Problem**: 100 nodes crash, all rejoin at once

**Mitigation**:
- Jittered join delay (random 0-5s)
- Exponential backoff on join failures
- Rate limit join requests on seed nodes

### 3. Gossip Amplification
**Problem**: Large cluster creates too many messages

**Mitigation**:
- Gossip fanout remains constant (default: 3)
- Piggyback updates on existing messages
- Limit update batch size

### 4. Stale Node IDs
**Problem**: Node restarts with same address, different NodeId

**Mitigation**:
- NodeId includes timestamp or incarnation
- Higher incarnation wins in conflict
- Remove dead nodes after TTL

### 5. Clock Skew
**Problem**: SystemTime differs across nodes

**Mitigation**:
- Use monotonic time (Instant) for local decisions
- Only use SystemTime for logging/metadata
- Don't rely on timestamp ordering

### 6. Slow Nodes
**Problem**: Node is alive but very slow (high latency)

**Mitigation**:
- Health checks include latency threshold
- Mark as unhealthy if latency > threshold
- Separate "alive" (responding) from "healthy" (performant)

---

## Performance Expectations

### Gossip Overhead

For N nodes:
- Each node sends `gossip_fanout` (3) messages per round
- Message size: ~500 bytes (includes node updates)
- Interval: 1s

**Total**: 3N messages/sec × 500 bytes = 1.5KB/sec per node

For 100 nodes: 150 KB/sec total cluster traffic

### Detection Time

SWIM guarantees:
- O(log N) rounds to detect failure
- With 1s gossip interval, 100 nodes: ~7 seconds

With phi accrual:
- Adaptive, typically 2-3× heartbeat interval
- Default 5s heartbeat: detect in 10-15s

### Memory Usage

Per node:
- ClusterNode: ~200 bytes
- NodeStatus: ~300 bytes
- Total: ~500 bytes per node

For 100 nodes: 50 KB

---

## Open Questions & Decisions Needed

### Q1: Cluster Port Reuse?
Should cluster port be optional, reusing RPC port?

**Option A**: Separate ports (proposed)
- ✅ Security boundary
- ✅ Separate firewall rules
- ❌ More port management

**Option B**: Same port
- ✅ Simpler configuration
- ❌ Mixes traffic types

**Decision**: Separate ports (better for production)

### Q2: Tag Update Frequency?
How often can tags be updated?

**Option A**: On-demand via API
- ✅ Immediate updates
- ❌ Creates update messages

**Option B**: Gossiped with heartbeat
- ✅ No extra messages
- ❌ Eventual consistency

**Decision**: Option A (on-demand), piggybacked on next gossip

### Q3: Dead Node Storage?
Keep history of dead nodes?

**Option A**: Remove after TTL
- ✅ Bounded memory
- ❌ Lose history

**Option B**: Persist to disk
- ✅ Debugging
- ❌ Complexity

**Decision**: Option A (remove after TTL), add optional logging

### Q4: Seed Node Failover?
What if all seed nodes are down?

**Current**: Return error `NoSeedsReachable`
**Alternative**: Cache peer list locally, try cached peers

**Decision**: Start with error, add cache in future if needed

---

## Success Metrics

### Functional
- ✅ connection_swap works without manual registry code
- ✅ Nodes discover each other via gossip
- ✅ Failures detected within 2× health_interval
- ✅ Graceful leave removes node immediately
- ✅ Tag-based queries work
- ✅ Cluster events emitted correctly

### Performance
- ✅ Gossip overhead < 1% CPU on 100-node cluster
- ✅ Memory usage < 1MB for 100 nodes
- ✅ Failure detection < 20 seconds (p99)
- ✅ Join time < 5 seconds

### Reliability
- ✅ Handles 50% node failures
- ✅ Handles network partitions gracefully
- ✅ No split brain for < 3-node clusters
- ✅ Eventual consistency (all nodes see same state within 10s)

---

## Non-Goals (Out of Scope)

1. **Consensus** - Not implementing Raft/Paxos (use external coordination if needed)
2. **Persistence** - Cluster state is in-memory only
3. **Encryption** - Cluster messages not encrypted (use VPN/mTLS)
4. **Authentication** - No auth on cluster port (firewall it)
5. **WAN Support** - Optimized for LAN (< 10ms latency)
6. **Leader Election** - No leader, fully p2p
7. **State Transfer** - Only membership, not application state

---

## Dependencies

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

[dev-dependencies]
tokio-test = "0.4"
```

---

## Conclusion

This design provides:
1. **Clear scope**: SWIM gossip + phi accrual failure detection
2. **Concrete APIs**: Full type signatures, no pseudocode
3. **Migration path**: Exact code changes for connection_swap
4. **Edge cases**: 6 failure scenarios with mitigations
5. **Testing**: Unit + integration test plans
6. **Performance**: Bounded overhead, concrete numbers

**Ready to implement?** YES - all ambiguities resolved.

**Estimated effort**: 6 weeks (one phase per week, overlapping)

**Risk level**: MEDIUM
- Gossip protocol is well-understood
- QUIC transport is familiar (already used)
- Main risk: integration with existing RpcServer
