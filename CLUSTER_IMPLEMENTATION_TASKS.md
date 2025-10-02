# Cluster Framework Implementation Tasks

Tasks organized by linear dependency - each layer depends on completion of previous layers.

---

## Layer 1: Foundation (No Dependencies)

### 1.1: Create cluster module structure
Create `/src/cluster/mod.rs` with submodules:
- `config.rs` - Cluster configuration
- `node.rs` - Node representation
- `error.rs` - Cluster-specific errors
- `gossip/` - SWIM gossip protocol
- `health/` - Health checking
- `discovery/` - Service discovery

**Acceptance Criteria:**
- Module structure exists and compiles
- Public exports defined in `mod.rs`

### 1.2: Implement ClusterError
Create `/src/cluster/error.rs` with error types:
```rust
pub enum ClusterError {
    NodeNotFound(NodeId),
    NetworkError(String),
    SerializationError(bincode::Error),
    HealthCheckTimeout,
    InvalidConfiguration(String),
}
```

**Acceptance Criteria:**
- All error variants defined
- Implements `std::error::Error` and `Display`
- Converts from underlying errors (bincode, io::Error)

### 1.3: Implement NodeId and ClusterNode
Create `/src/cluster/node.rs`:
```rust
pub struct NodeId(Uuid);
pub struct ClusterNode {
    id: NodeId,
    addr: SocketAddr,
    tags: HashMap<String, String>,
    state: NodeState,
    incarnation: u64,
}
pub enum NodeState { Alive, Suspect, Dead }
```

**Acceptance Criteria:**
- NodeId is unique, serializable, hashable
- ClusterNode tracks all required fields
- NodeState transitions defined

### 1.4: Implement ClusterConfig
Create `/src/cluster/config.rs`:
```rust
pub struct ClusterConfig {
    pub node_id: Option<NodeId>,
    pub cluster_addr: SocketAddr,
    pub seed_nodes: Vec<SocketAddr>,
    pub tags: HashMap<String, String>,
    pub gossip_interval: Duration,
    pub suspicion_timeout: Duration,
    pub phi_threshold: f64,
}
```

**Acceptance Criteria:**
- Config has sensible defaults
- Builder pattern for optional fields
- Validates config on build

---

## Layer 2: Core Types (Depends on Layer 1)

### 2.1: Define gossip message types
Create `/src/cluster/gossip/messages.rs`:
```rust
pub enum GossipMessage {
    Ping { from: NodeId, incarnation: u64 },
    Ack { from: NodeId },
    IndirectPing { target: NodeId, requestor: NodeId },
    StateUpdate { nodes: Vec<NodeStateUpdate> },
}
pub struct NodeStateUpdate {
    node_id: NodeId,
    state: NodeState,
    incarnation: u64,
}
```

**Acceptance Criteria:**
- All SWIM protocol messages defined
- Implements Serialize/Deserialize
- Message size is reasonable (<1KB typical)

### 2.2: Implement NodeRegistry
Create `/src/cluster/gossip/registry.rs`:
```rust
pub struct NodeRegistry {
    nodes: RwLock<HashMap<NodeId, ClusterNode>>,
}
impl NodeRegistry {
    pub fn add_node(&self, node: ClusterNode);
    pub fn update_state(&self, id: NodeId, state: NodeState, incarnation: u64);
    pub fn get_alive_nodes(&self) -> Vec<ClusterNode>;
    pub fn get_nodes_by_tag(&self, key: &str, value: &str) -> Vec<ClusterNode>;
}
```

**Acceptance Criteria:**
- Thread-safe node storage
- Efficient tag-based queries
- Handles state transitions correctly
- Unit tests for concurrent access

### 2.3: Define HealthCheck trait
Create `/src/cluster/health/mod.rs`:
```rust
#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;
}
pub struct HealthStatus {
    pub healthy: bool,
    pub message: Option<String>,
    pub metadata: HashMap<String, String>,
}
```

**Acceptance Criteria:**
- Trait is async and thread-safe
- HealthStatus carries sufficient info
- Default implementation for "always healthy"

---

## Layer 3: Detection & Monitoring (Depends on Layer 2)

### 3.1: Implement Phi Accrual Failure Detector
Create `/src/cluster/gossip/phi_detector.rs`:
```rust
pub struct PhiAccrualDetector {
    window_size: usize,
    phi_threshold: f64,
    arrivals: VecDeque<Instant>,
}
impl PhiAccrualDetector {
    pub fn heartbeat(&mut self);
    pub fn phi(&self) -> f64;
    pub fn is_available(&self) -> bool;
}
```

**Acceptance Criteria:**
- Implements phi calculation from SWIM paper
- Maintains sliding window of heartbeat arrivals
- Returns suspicion level as continuous value
- Unit tests verify phi increases over time without heartbeats
- Unit tests verify phi resets on heartbeat

### 3.2: Implement HealthCheckService
Create `/src/cluster/health/service.rs`:
```rust
pub struct HealthCheckService {
    checks: Vec<Box<dyn HealthCheck>>,
    interval: Duration,
}
impl HealthCheckService {
    pub fn add_check(&mut self, check: Box<dyn HealthCheck>);
    pub async fn run_checks(&self) -> HealthStatus;
    pub fn start_monitoring(&self) -> JoinHandle<()>;
}
```

**Acceptance Criteria:**
- Runs all registered checks in parallel
- Aggregates results (healthy only if all pass)
- Background task runs checks periodically
- Can be stopped gracefully

---

## Layer 4: Gossip Protocol (Depends on Layer 3)

### 4.1: Implement GossipTransport
Create `/src/cluster/gossip/transport.rs`:
```rust
pub struct GossipTransport {
    socket: Arc<UdpSocket>,
}
impl GossipTransport {
    pub async fn send(&self, addr: SocketAddr, msg: GossipMessage) -> Result<()>;
    pub async fn recv(&self) -> Result<(SocketAddr, GossipMessage)>;
}
```

**Acceptance Criteria:**
- UDP-based message transport
- Serializes/deserializes messages with bincode
- Handles network errors gracefully
- Logs send/recv at DEBUG level

### 4.2: Implement SWIM ping/ack logic
Create `/src/cluster/gossip/swim.rs`:
```rust
pub struct SwimProtocol {
    registry: Arc<NodeRegistry>,
    transport: Arc<GossipTransport>,
    local_node: ClusterNode,
    phi_detectors: RwLock<HashMap<NodeId, PhiAccrualDetector>>,
}
impl SwimProtocol {
    async fn direct_ping(&self, target: &ClusterNode) -> Result<Duration>;
    async fn indirect_ping(&self, target: &ClusterNode) -> Result<()>;
    async fn handle_ping(&self, from: NodeId, incarnation: u64);
    async fn handle_ack(&self, from: NodeId);
}
```

**Acceptance Criteria:**
- Direct ping with timeout (1s)
- Indirect ping through K random nodes (K=3)
- Updates phi detector on successful ping
- Marks node as suspect after ping failures
- Unit tests for ping/ack roundtrip
- Unit tests for indirect ping fallback

### 4.3: Implement state dissemination
Add to `/src/cluster/gossip/swim.rs`:
```rust
impl SwimProtocol {
    fn piggyback_state(&self, target: &ClusterNode) -> Vec<NodeStateUpdate>;
    async fn handle_state_update(&self, updates: Vec<NodeStateUpdate>);
}
```

**Acceptance Criteria:**
- Piggybacks up to 10 state updates on each ping
- Prioritizes recent state changes
- Applies updates with incarnation number checks
- Tracks propagation count to limit gossip
- Unit tests verify state convergence

---

## Layer 5: Gossip Service (Depends on Layer 4)

### 5.1: Implement GossipService
Create `/src/cluster/gossip/service.rs`:
```rust
pub struct GossipService {
    swim: Arc<SwimProtocol>,
    config: ClusterConfig,
    shutdown: Arc<AtomicBool>,
}
impl GossipService {
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self);
    async fn gossip_loop(&self);
    async fn receive_loop(&self);
    async fn probe_loop(&self);
}
```

**Acceptance Criteria:**
- Three background tasks: gossip, receive, probe
- Gossip loop picks random node every interval
- Receive loop handles incoming messages
- Probe loop runs failure detection
- Graceful shutdown on stop()
- Integration test with 3 nodes forming cluster

---

## Layer 6: RpcServer Integration (Depends on Layer 5)

### 6.1: Add cluster field to RpcServer
Modify `/src/server.rs`:
```rust
pub struct RpcServer {
    // existing fields...
    cluster: Option<Arc<Cluster>>,
}
impl RpcServer {
    pub fn with_cluster(mut self, config: ClusterConfig) -> Self {
        self.cluster = Some(Arc::new(Cluster::new(config)));
        self
    }
}
```

**Acceptance Criteria:**
- RpcServer stores optional Cluster
- No behavioral change when cluster is None
- Cluster is initialized but not started yet

### 6.2: Implement Cluster struct
Create `/src/cluster/cluster.rs`:
```rust
pub struct Cluster {
    config: ClusterConfig,
    registry: Arc<NodeRegistry>,
    gossip: Arc<GossipService>,
    health: Arc<HealthCheckService>,
}
impl Cluster {
    pub fn new(config: ClusterConfig) -> Self;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self);
    pub fn local_node(&self) -> &ClusterNode;
    pub fn get_nodes_by_tag(&self, key: &str, value: &str) -> Vec<ClusterNode>;
}
```

**Acceptance Criteria:**
- Owns all cluster components
- start() launches gossip and health services
- stop() gracefully shuts down all services
- Exposes query methods for service discovery

### 6.3: Start cluster on RpcServer::start()
Modify `/src/server.rs`:
```rust
impl RpcServer {
    pub async fn start(&mut self, quic: Endpoint) -> Result<()> {
        if let Some(cluster) = &self.cluster {
            cluster.start().await?;
            info!("cluster started on {}", cluster.config().cluster_addr);
        }
        // existing server logic...
    }
}
```

**Acceptance Criteria:**
- Cluster starts before RPC server
- Cluster port is separate from RPC port
- Errors propagate correctly
- Integration test: RpcServer with cluster enabled

### 6.4: Stop cluster on RpcServer shutdown
Modify `/src/server.rs` shutdown logic:
```rust
impl RpcServer {
    async fn shutdown(&self) {
        if let Some(cluster) = &self.cluster {
            cluster.stop().await;
            info!("cluster stopped");
        }
        // existing shutdown logic...
    }
}
```

**Acceptance Criteria:**
- Cluster stops gracefully
- No panics on shutdown
- Integration test: start and stop multiple times

---

## Layer 7: Service Discovery API (Depends on Layer 6)

### 7.1: Add query methods to RpcServer
Modify `/src/server.rs`:
```rust
impl RpcServer {
    pub fn cluster(&self) -> Option<&Cluster> {
        self.cluster.as_deref()
    }
    
    pub fn discover_nodes(&self, tags: &[(&str, &str)]) -> Vec<ClusterNode> {
        self.cluster
            .as_ref()
            .map(|c| c.query_by_tags(tags))
            .unwrap_or_default()
    }
}
```

**Acceptance Criteria:**
- Can query nodes from RpcServer
- Returns empty vec when cluster disabled
- Integration test: discover nodes by tag

### 7.2: Implement ClusterNode::connect()
Add to `/src/cluster/node.rs`:
```rust
impl ClusterNode {
    pub async fn connect(&self, config: RpcConfig) -> Result<RpcClient> {
        RpcClient::connect(self.addr, config).await
    }
}
```

**Acceptance Criteria:**
- Creates RpcClient to node's RPC address
- Uses provided TLS config
- Returns connection errors
- Integration test: discover and connect

---

## Layer 8: Connection Swap Migration (Depends on Layer 7)

### 8.1: Simplify director registration logic
Modify `examples/connection_swap/src/bin/director.rs`:
```rust
let config = RpcConfig::new("../../certs/test_cert.pem", user_addr)
    .with_key_path("../../certs/test_key.pem")
    .with_cluster(ClusterConfig::new(mgmt_addr)
        .with_tags([("role", "director")]));

let mut server = RpcServer::new(config);
```

**Acceptance Criteria:**
- Remove manual MGMT port server
- Remove register_worker RPC endpoint
- Director uses cluster for worker discovery
- Integration test: director discovers workers

### 8.2: Simplify worker registration logic
Modify `examples/connection_swap/src/bin/worker.rs`:
```rust
let config = RpcConfig::new("../../certs/test_cert.pem", user_addr)
    .with_key_path("../../certs/test_key.pem")
    .with_cluster(ClusterConfig::new(mgmt_addr)
        .with_tags([
            ("role", "worker"),
            ("label", &worker_label),
        ])
        .with_seed_nodes(vec![director_mgmt_addr]));

let mut server = RpcServer::new(config);
server.cluster().unwrap().add_health_check(Box::new(AvailabilityCheck::new(is_available)));
```

**Acceptance Criteria:**
- Remove manual MGMT port server
- Remove registration loop
- Worker joins cluster via seed nodes
- Custom health check reflects availability
- Integration test: worker joins and leaves cluster

### 8.3: Update director worker assignment
Modify `examples/connection_swap/src/bin/director.rs`:
```rust
async fn assign_worker(&self, server: &RpcServer) -> Option<ClusterNode> {
    let workers = server.discover_nodes(&[
        ("role", "worker"),
        ("healthy", "true"),
    ]);
    
    if workers.is_empty() {
        return None;
    }
    
    let idx = self.next_worker_idx.fetch_add(1, Ordering::SeqCst);
    Some(workers[idx % workers.len()].clone())
}
```

**Acceptance Criteria:**
- Discovers workers via cluster API
- Only assigns to healthy workers
- Round-robin still works
- Integration test: director load-balances across workers

### 8.4: Update director migration logic
Modify `examples/connection_swap/src/bin/director.rs`:
```rust
async fn handle_worker_error(&self, server: &RpcServer, failed_worker: &str) -> Option<ClusterNode> {
    warn!(worker = failed_worker, "worker failed - finding replacement");
    
    let workers = server.discover_nodes(&[
        ("role", "worker"),
        ("healthy", "true"),
    ]);
    
    workers.into_iter()
        .find(|w| w.tags.get("label") != Some(&failed_worker.to_string()))
}
```

**Acceptance Criteria:**
- Finds replacement worker via cluster
- Excludes failed worker
- Returns None if no healthy workers
- Integration test: director migrates on worker failure

### 8.5: Remove manual health check code
Cleanup in both director and worker:
- Remove WorkerInfo struct
- Remove RegisterWorkerRequest/Response
- Remove health_check RPC endpoint
- Remove manual heartbeat loops

**Acceptance Criteria:**
- All manual clustering code removed
- Example only uses cluster framework
- All tests still pass
- run_demo.sh works end-to-end

---

## Layer 9: Documentation & Testing (Depends on Layer 8)

### 9.1: Add cluster framework documentation
Create `/src/cluster/README.md`:
- Overview of cluster framework
- SWIM protocol explanation
- Usage examples with RpcServer
- Configuration options
- Performance characteristics

**Acceptance Criteria:**
- Clear examples of cluster usage
- Explains tag-based discovery
- Documents health check integration

### 9.2: Add cluster framework tests
Create `/src/cluster/tests/`:
- `test_node_registry.rs` - concurrent access, state transitions
- `test_phi_detector.rs` - phi calculation, thresholds
- `test_swim.rs` - ping/ack, indirect ping, state dissemination
- `test_service_discovery.rs` - tag queries, filtering

**Acceptance Criteria:**
- All edge cases covered
- Tests are deterministic
- No flaky timing issues
- All tests pass in CI

### 9.3: Add integration tests
Create `/tests/cluster_integration.rs`:
- 3-node cluster formation
- Node failure detection
- State convergence
- Split brain scenario
- Graceful shutdown

**Acceptance Criteria:**
- Tests use real UDP sockets
- Tests verify convergence times
- Tests clean up resources
- All tests pass reliably

### 9.4: Update connection_swap documentation
Update `examples/connection_swap/README.md`:
- Remove references to manual MGMT port
- Document cluster framework integration
- Update architecture diagram
- Update expected output
- Mark "gossip support" as completed

**Acceptance Criteria:**
- README matches new implementation
- Architecture diagram shows cluster framework
- Quick start still works

### 9.5: Add migration guide
Create `/docs/CLUSTER_MIGRATION.md`:
- How to migrate from manual management ports
- Before/after code examples
- Configuration migration
- Health check integration patterns

**Acceptance Criteria:**
- Step-by-step migration guide
- Real code examples
- Covers common patterns

---

## Summary

**Total Tasks: 35**

**Dependency Layers:**
- Layer 1: 4 tasks (foundation)
- Layer 2: 3 tasks (core types)
- Layer 3: 2 tasks (detection)
- Layer 4: 3 tasks (gossip protocol)
- Layer 5: 1 task (gossip service)
- Layer 6: 4 tasks (server integration)
- Layer 7: 2 tasks (service discovery)
- Layer 8: 5 tasks (connection_swap migration)
- Layer 9: 5 tasks (docs & tests)

**Critical Path:**
1.1 → 1.2 → 1.3 → 1.4 → 2.1 → 2.2 → 2.3 → 3.1 → 3.2 → 4.1 → 4.2 → 4.3 → 5.1 → 6.1 → 6.2 → 6.3 → 6.4 → 7.1 → 7.2 → 8.1 → 8.2 → 8.3 → 8.4 → 8.5 → 9.4

**Parallelization Opportunities:**
- Layer 1 tasks can run in parallel
- Layer 2 tasks can run in parallel
- Layer 9 tasks can run in parallel
- Tests (9.2, 9.3) can run while docs (9.1, 9.4, 9.5) are written
