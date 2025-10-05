# Cluster Implementation Tasks

**Task organization**: Dependency-based layers. Each layer can be implemented in parallel, but must complete before the next layer begins.

---

## Layer 1: Foundation Components (No Dependencies)

### Task 1.1: Connection Pool Infrastructure

**Dependencies**: None

**Files**:
- Create `src/cluster/connection_pool.rs`
- Create `src/cluster/connection_pool/config.rs`
- Create `src/cluster/connection_pool/error.rs`

**Trait Boundary**:
```rust
pub trait ConnectionPool: Send + Sync {
    async fn get_or_create(&self, addr: SocketAddr) -> Result<PooledConnection, PoolError>;
    fn release(&self, addr: &SocketAddr);
    async fn run_idle_cleanup(&self);
    fn stats(&self) -> PoolStats;
}

pub struct PooledConnection {
    pub(crate) connection: s2n_quic::Connection,
    pub(crate) addr: SocketAddr,
    pub(crate) last_used: Instant,
}
```

**Acceptance Criteria**:
- DashMap-based pool with max 50 connections
- Per-peer limit (default 1)
- Idle connections evicted after 60s
- Connection reuse working
- Health check loop runs every 30s
- Tests: reuse, eviction, max bounds, concurrent access

---

### Task 1.2: Incarnation Number System

**Dependencies**: None

**Files**:
- Create `src/cluster/incarnation.rs`

**Trait Boundary**:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incarnation(u64);

impl Incarnation {
    pub fn initial() -> Self;
    pub fn increment(&mut self);
    pub fn compare(&self, other: &Incarnation) -> Ordering;
}

pub trait IncarnationResolver {
    fn resolve_conflict<'a>(&self, a: &'a NodeStatus, b: &'a NodeStatus) -> &'a NodeStatus;
}
```

**Acceptance Criteria**:
- Timestamp-based initial incarnation
- Wraparound-safe comparison (handles u64 overflow)
- NodeId tiebreaker for equal incarnations
- Refutation increments by 1
- Tests: wraparound edge cases, tiebreaker determinism, concurrent increments

---

### Task 1.3: Gossip Message Size Bounds

**Dependencies**: None

**Files**:
- Create `src/cluster/gossip/message.rs`
- Create `src/cluster/gossip/queue.rs`

**Trait Boundary**:
```rust
pub trait GossipQueue: Send + Sync {
    fn enqueue(&mut self, update: NodeUpdate, priority: Priority);
    fn select_updates(&mut self) -> Vec<NodeUpdate>;
    fn mark_sent(&mut self, node_id: &NodeId);
    fn should_stop_gossiping(&self, node_id: &NodeId) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

const MAX_UPDATES_PER_MESSAGE: usize = 20;
const MAX_MESSAGE_SIZE: usize = 4096;
```

**Acceptance Criteria**:
- BTreeMap-based priority queue
- Max 20 updates per message
- Hard 4KB message size check
- Stop gossiping after log(N) * 3 rounds
- Priority-based selection (critical first)
- Tests: size bounds enforced, priority ordering, redundancy elimination

---

### Task 1.4: Phi Accrual Failure Detector

**Dependencies**: None

**Files**:
- Create `src/cluster/failure_detection/phi_accrual.rs`
- Add `statrs = "0.17"` to Cargo.toml

**Trait Boundary**:
```rust
pub trait FailureDetector: Send + Sync {
    fn heartbeat(&mut self);
    fn phi(&self) -> f64;
    fn is_available(&self, threshold: f64) -> bool;
    fn clear(&mut self);
}

pub struct PhiAccrualDetector {
    history: VecDeque<f64>,
    max_samples: usize,
    threshold: f64,
    last_heartbeat: Instant,
    min_samples: usize,
}
```

**Acceptance Criteria**:
- Correct CDF calculation using statrs::Normal
- Min 5 samples before reporting phi
- VecDeque window (max 100 samples)
- Correct phi = -log10(P) formula
- Tests: phi increases with time, normal distribution edge cases, min samples requirement

---

### Task 1.5: Partition Detector

**Dependencies**: None

**Files**:
- Create `src/cluster/partition_detector.rs`

**Trait Boundary**:
```rust
pub trait PartitionDetector: Send + Sync {
    async fn check_partition(&self, current_size: usize) -> PartitionStatus;
    fn update_expected_size(&self, size: usize);
    fn config(&self) -> &PartitionConfig;
}

pub enum PartitionStatus {
    Unknown,
    Healthy { current_size: usize, expected_size: usize },
    Suspect { current_size: usize, expected_size: usize, grace_remaining: Duration },
    Partitioned { current_size: usize, expected_size: usize, minority: bool },
}

pub struct PartitionConfig {
    pub threshold: f64,
    pub grace_period: Duration,
    pub read_only_on_partition: bool,
}
```

**Acceptance Criteria**:
- Quorum threshold (default 50%)
- Grace period (default 30s)
- Suspect state before confirmed partition
- AtomicUsize for expected size
- Tests: quorum scenarios (49%, 50%, 51%), grace period timing, minority detection

---

### Task 1.6: Event Channel with Backpressure Tracking

**Dependencies**: None

**Files**:
- Create `src/cluster/events.rs`

**Trait Boundary**:
```rust
pub struct ClusterEventBroadcaster {
    tx: broadcast::Sender<ClusterEvent>,
    drops: Arc<AtomicU64>,
}

pub struct ClusterEventReceiver {
    inner: broadcast::Receiver<ClusterEvent>,
    drops: Arc<AtomicU64>,
}

pub enum RecvError {
    Lagged(u64),
    Closed,
}

impl ClusterEventReceiver {
    pub async fn recv(&mut self) -> Result<ClusterEvent, RecvError>;
    pub fn dropped_count(&self) -> u64;
}

pub enum ClusterEvent {
    NodeJoined(ClusterNode),
    NodeLeft(NodeId),
    NodeFailed(NodeId),
    NodeRecovered(NodeId),
    NodeTagsUpdated { node_id: NodeId, tags: HashMap<String, String> },
    PartitionDetected { status: PartitionStatus },
    EventsDropped { count: u64 },
}

const EVENT_CHANNEL_CAPACITY: usize = 1000;
```

**Acceptance Criteria**:
- Bounded broadcast channel (1000 capacity)
- Track dropped events in AtomicU64
- Emit EventsDropped event when consumer lags
- RecvError::Lagged contains drop count
- Tests: backpressure behavior, drop tracking, multiple subscribers

---

## Layer 2: Core Cluster Components (Depends on Layer 1)

### Task 2.1: Node Registry with Shared Ownership

**Dependencies**: 
- Task 1.2 (Incarnation)

**Files**:
- Create `src/cluster/node_registry.rs`

**Trait Boundary**:
```rust
pub trait NodeRegistry: Send + Sync {
    fn insert(&self, status: NodeStatus);
    fn get(&self, node_id: &NodeId) -> Option<NodeStatus>;
    fn remove(&self, node_id: &NodeId) -> Option<NodeStatus>;
    fn all_nodes(&self) -> Vec<NodeStatus>;
    fn alive_nodes(&self) -> Vec<NodeStatus>;
    fn len(&self) -> usize;
}

pub struct SharedNodeRegistry {
    inner: Arc<RwLock<HashMap<NodeId, NodeStatus>>>,
}

pub struct NodeStatus {
    pub node: ClusterNode,
    pub state: NodeState,
    pub incarnation: Incarnation,
    pub last_seen: Instant,
}

pub enum NodeState {
    Alive,
    Suspect,
    Failed,
    Left,
}
```

**Acceptance Criteria**:
- Arc<RwLock<HashMap>> for shared access
- Incarnation-based conflict resolution
- State transitions (Alive → Suspect → Failed)
- Clone is cheap (Arc clone)
- Tests: concurrent access, conflict resolution, state transitions

---

### Task 2.2: Gossip Protocol Core

**Dependencies**:
- Task 1.1 (Connection Pool)
- Task 1.2 (Incarnation)
- Task 1.3 (Message Bounds)
- Task 2.1 (Node Registry)

**Files**:
- Create `src/cluster/gossip/protocol.rs`
- Create `src/cluster/gossip/ping.rs`
- Create `src/cluster/gossip/ack.rs`

**Trait Boundary**:
```rust
pub trait GossipProtocol: Send + Sync {
    async fn start(&self) -> Result<(), GossipError>;
    async fn stop(&self);
    async fn broadcast(&self, update: NodeUpdate, priority: Priority);
    fn select_random_nodes(&self, count: usize) -> Vec<ClusterNode>;
}

pub struct GossipConfig {
    pub protocol_period: Duration,
    pub indirect_ping_count: usize,
    pub ack_timeout: Duration,
    pub indirect_timeout: Duration,
}
```

**Acceptance Criteria**:
- Direct ping/ack cycle
- Indirect ping (K=3 intermediaries)
- Connection pool reuse
- Gossip queue integration (20 updates/msg, 4KB max)
- Incarnation conflict resolution
- Tests: direct ping, indirect ping, message bounds, connection reuse

---

### Task 2.3: Health Checker with Phi Accrual

**Dependencies**:
- Task 1.4 (Phi Accrual)
- Task 2.1 (Node Registry)

**Files**:
- Create `src/cluster/health_checker.rs`
- Create `src/cluster/health_check/trait.rs`

**Trait Boundary**:
```rust
pub trait HealthChecker: Send + Sync {
    async fn start(&self);
    async fn stop(&self);
    fn register_check(&mut self, check: Box<dyn HealthCheck>);
    fn status(&self) -> HealthStatus;
}

pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthCheckResult;
    fn name(&self) -> &str;
}

pub struct HealthStatus {
    pub overall: CheckStatus,
    pub checks: Vec<(String, CheckStatus)>,
}

pub enum CheckStatus {
    Healthy,
    Degraded,
    Unhealthy,
}
```

**Acceptance Criteria**:
- Shared reference to node registry (not owned)
- Run checks every N seconds (configurable)
- Emit HealthCheckFailed events
- Update node states (Alive → Suspect)
- Phi accrual per-node tracking
- Tests: phi threshold triggering, event emission, shared registry access

---

### Task 2.4: Graceful Shutdown Protocol

**Dependencies**:
- Task 2.2 (Gossip Protocol)
- Task 2.3 (Health Checker)

**Files**:
- Modify `src/cluster/membership.rs` (create if needed)

**Trait Boundary**:
```rust
pub trait Lifecycle: Send + Sync {
    async fn leave(&mut self) -> Result<(), ClusterError>;
    async fn shutdown_immediate(&mut self);
}

impl ClusterMembership {
    pub async fn leave(&mut self) -> Result<(), ClusterError>;
}
```

**Acceptance Criteria**:
- Phase 1: Broadcast Leave (max 5s timeout)
- Phase 2: Stop gossip loop
- Phase 3: Stop health checker
- Phase 4: Close QUIC connections (max 2s timeout)
- Phase 5: Clear node registry
- Phase 6: Close event channel
- Drop impl sends shutdown signal
- Tests: all phases complete, timeouts enforced, force shutdown works

---

## Layer 3: Tag System and Service Discovery (Depends on Layer 2)

### Task 3.1: Tag-Based Node Discovery

**Dependencies**:
- Task 2.1 (Node Registry)
- Task 2.2 (Gossip Protocol)

**Files**:
- Create `src/cluster/tags.rs`
- Modify `src/cluster/membership.rs`

**Trait Boundary**:
```rust
pub trait ServiceDiscovery: Send + Sync {
    async fn update_tag(&self, key: String, value: String);
    async fn remove_tag(&self, key: &str);
    async fn nodes_with_tag(&self, key: &str, value: &str) -> Vec<ClusterNode>;
    async fn nodes_with_all_tags(&self, tags: &HashMap<String, String>) -> Vec<ClusterNode>;
}

impl ClusterMembership {
    pub async fn update_tag(&self, key: String, value: String);
    pub async fn nodes_with_tag(&self, key: &str, value: &str) -> Vec<ClusterNode>;
}
```

**Acceptance Criteria**:
- Eventual consistency model (no synchronous guarantees)
- Tags stored in ClusterNode
- Broadcast tag updates with High priority
- Emit NodeTagsUpdated event
- Local cache for fast queries
- Tests: tag propagation, query results, eventual consistency

---

### Task 3.2: Director Worker Assignment with Retry

**Dependencies**:
- Task 3.1 (Tag Discovery)

**Files**:
- Create example in `examples/director_with_cluster/director.rs`

**Trait Boundary**:
```rust
pub trait WorkerAssigner: Send + Sync {
    async fn assign_worker(&self) -> Option<WorkerAssignment>;
}

pub struct WorkerAssignment {
    pub addr: SocketAddr,
    pub node_id: NodeId,
}
```

**Acceptance Criteria**:
- Query nodes with tag role=worker
- Filter by available=true
- Retry up to 3 times with 500ms delay
- Return None if no workers available
- Round-robin selection
- Tests: retry logic, filtering, no-workers scenario

---

## Layer 4: RpcServer Integration (Depends on Layer 3)

### Task 4.1: Cluster Module in RpcServer

**Dependencies**:
- Task 2.1 (Node Registry)
- Task 2.2 (Gossip Protocol)
- Task 2.3 (Health Checker)
- Task 2.4 (Shutdown Protocol)
- Task 3.1 (Tag Discovery)

**Files**:
- Modify `src/server.rs`
- Create `src/cluster/mod.rs` (if not exists)

**Trait Boundary**:
```rust
impl RpcServer {
    pub fn enable_cluster(
        &mut self,
        config: ClusterConfig,
        seeds: Vec<SocketAddr>,
    ) -> Result<(), ClusterError>;
    
    pub fn cluster(&self) -> Option<&ClusterMembership>;
    
    pub async fn update_tag(&self, key: String, value: String) -> Result<(), ClusterError>;
    
    pub fn cluster_events(&self) -> Option<ClusterEventReceiver>;
}

pub struct ClusterConfig {
    pub node_id: Option<NodeId>,
    pub gossip: GossipConfig,
    pub health: HealthConfig,
    pub partition: PartitionConfig,
}
```

**Acceptance Criteria**:
- ClusterMembership as Option<Arc<>> in RpcServer
- Auto-join on enable_cluster()
- Auto-leave on server drop
- Health checker starts automatically
- Event receiver cloneable
- Tests: enable/disable, auto-join, auto-leave

---

### Task 4.2: Bootstrap with Seed Retry

**Dependencies**:
- Task 4.1 (RpcServer Integration)

**Files**:
- Modify `src/cluster/membership.rs`
- Add `rand = "0.8"` to Cargo.toml

**Trait Boundary**:
```rust
impl ClusterMembership {
    pub async fn join(
        config: ClusterConfig,
        seeds: Vec<SocketAddr>,
    ) -> Result<Self, ClusterError>;
    
    async fn join_with_retry(
        gossip: &Arc<GossipProtocol>,
        seeds: &[SocketAddr],
    ) -> Result<(), ClusterError>;
}

pub enum ClusterError {
    NoSeedsReachable,
    JoinTimeout,
}
```

**Acceptance Criteria**:
- Max 10 retry attempts
- Exponential backoff (100ms → 10s)
- Random jitter (±25%)
- Try all seeds per attempt
- Return error after max attempts
- Tests: retry logic, backoff timing, jitter randomness, success on N-th attempt

---

## Layer 5: End-to-End Examples and Testing (Depends on Layer 4)

### Task 5.1: Connection Swap with Cluster Discovery

**Dependencies**:
- Task 4.1 (RpcServer Integration)
- Task 4.2 (Seed Bootstrap)
- Task 3.2 (Director Assignment)

**Files**:
- Modify `examples/connection_swap/director.rs`
- Modify `examples/connection_swap/worker.rs`
- Modify `examples/connection_swap/client.rs`

**Trait Boundary**:
No new traits (uses existing ServiceDiscovery)

**Acceptance Criteria**:
- Director uses cluster for worker discovery (remove static registry)
- Workers register via cluster tags (role=worker, available=true/false)
- Client queries director → director queries cluster
- Failover still works (worker marks available=false on failure)
- Worker recovery updates available=true
- Tests: end-to-end failover, tag propagation delay handling

---

### Task 5.2: Chaos Testing Suite

**Dependencies**:
- Task 5.1 (Connection Swap)

**Files**:
- Create `tests/chaos/mod.rs`
- Create `tests/chaos/network_partition.rs`
- Create `tests/chaos/cascading_failures.rs`
- Create `tests/chaos/message_loss.rs`

**Trait Boundary**:
```rust
pub trait ChaosScenario {
    fn name(&self) -> &str;
    async fn setup(&mut self) -> Result<TestCluster, ChaosError>;
    async fn inject_chaos(&mut self, cluster: &mut TestCluster);
    async fn verify(&self, cluster: &TestCluster) -> Result<(), ChaosError>;
}

pub struct TestCluster {
    pub nodes: Vec<RpcServer>,
    pub control: ClusterControl,
}

pub struct ClusterControl {
    pub partition: PartitionControl,
    pub message_loss: MessageLossControl,
}
```

**Acceptance Criteria**:
- Network partition test (split 5-node cluster 3-2)
- Cascading failure test (3/5 nodes fail simultaneously)
- Message loss test (20% gossip messages dropped)
- Seed unreachable test (all seeds fail → backoff → retry)
- Tag update race test (concurrent tag updates)
- Each scenario has assertions for expected behavior
- Tests run in CI

---

### Task 5.3: Performance Benchmarks

**Dependencies**:
- Task 5.1 (Connection Swap)

**Files**:
- Create `benches/cluster.rs`

**Trait Boundary**:
No new traits (uses Criterion)

**Acceptance Criteria**:
- Benchmark gossip throughput (updates/sec)
- Benchmark tag query latency (P50, P95, P99)
- Benchmark connection pool reuse (vs new connections)
- Benchmark failure detection time (phi threshold trigger)
- Benchmark cluster size scalability (10, 50, 100, 500 nodes)
- Results documented in CLUSTER_DESIGN_V2.md

---

## Layer 6: Documentation and Polish

### Task 6.1: Update Documentation

**Dependencies**:
- All previous tasks

**Files**:
- Modify `README.md`
- Modify `CLUSTER_DESIGN_V2.md` (add benchmark results)
- Create `examples/cluster_basic/README.md`

**Acceptance Criteria**:
- README has cluster section with quick start
- CLUSTER_DESIGN_V2.md has actual benchmark data
- Basic cluster example with commented code
- Migration guide from connection_swap v1 → v2
- Architecture diagram (ASCII art)

---

### Task 6.2: API Cleanup and Stabilization

**Dependencies**:
- Task 6.1 (Documentation)

**Files**:
- Review all `pub` items in `src/cluster/`

**Acceptance Criteria**:
- Only essential types are public
- Internal types use `pub(crate)`
- No `pub` fields on structs (use methods)
- All public APIs have docs
- No compiler warnings
- Clippy passes

---

## Summary

**Total Layers**: 6  
**Total Tasks**: 20

**Layer Dependencies**:
- Layer 1 → Layer 2 → Layer 3 → Layer 4 → Layer 5 → Layer 6

**Parallel Opportunities**:
- Within Layer 1: All 6 tasks can run in parallel
- Within Layer 2: Tasks 2.1, 2.2, 2.3 can run in parallel (2.4 needs 2.2 and 2.3)
- Within Layer 3: Task 3.2 needs 3.1
- Within Layer 5: All 3 tasks can run in parallel after Layer 4

**Critical Path** (longest dependency chain):
1. Task 1.1 (Connection Pool)
2. Task 2.2 (Gossip Protocol)
3. Task 2.4 (Shutdown Protocol)
4. Task 4.1 (RpcServer Integration)
5. Task 4.2 (Bootstrap Retry)
6. Task 5.1 (Connection Swap)
7. Task 5.2 (Chaos Tests)

**Files Created/Modified**:
- ~25 new files in `src/cluster/`
- ~8 modified files in `src/`
- ~6 test files in `tests/chaos/`
- ~3 benchmark files in `benches/`
- ~4 example files modified

**External Dependencies Added**:
- `dashmap = "6"` (Task 1.1)
- `statrs = "0.17"` (Task 1.4)
- `rand = "0.8"` (Task 4.2)
