# Python Cluster API Design

## Goal
Expose RpcNet's SWIM-based cluster functionality to Python, enabling Python workers to participate in the cluster with automatic discovery, failure detection, and load balancing.

## Current Status
- ✅ Python bindings expose: `RpcServer`, `RpcClient`, serialization
- ❌ Python bindings DO NOT expose: cluster APIs, SWIM gossip, failure detection

## Required Components

### 1. Configuration Classes

#### `PyGossipConfig`
```python
class GossipConfig:
    """SWIM gossip protocol configuration"""
    def __init__(
        self,
        protocol_period_ms: int = 1000,
        ack_timeout_ms: int = 500,
        indirect_timeout_ms: int = 1000,
        suspicion_multiplier: int = 5,
    ): ...
```

**Rust mapping**: `cluster::GossipConfig`

#### `PyHealthCheckConfig`
```python
class HealthCheckConfig:
    """Phi Accrual failure detection configuration"""
    def __init__(
        self,
        check_interval_secs: int = 5,
        phi_threshold: float = 8.0,
    ): ...
```

**Rust mapping**: `cluster::HealthCheckConfig`

#### `PyPoolConfig`
```python
class PoolConfig:
    """Connection pool configuration"""
    def __init__(
        self,
        max_connections: int = 10,
        min_idle: int = 1,
        max_idle_secs: int = 300,
    ): ...
```

**Rust mapping**: `cluster::PoolConfig`

#### `PyClusterConfig`
```python
class ClusterConfig:
    """Complete cluster configuration"""
    def __init__(
        self,
        node_id: Optional[str] = None,
        gossip: GossipConfig = GossipConfig(),
        health: HealthCheckConfig = HealthCheckConfig(),
        pool: PoolConfig = PoolConfig(),
        bootstrap_timeout_secs: int = 30,
    ): ...
```

**Rust mapping**: `cluster::ClusterConfig`

### 2. QUIC Client Wrapper

#### `PyQuicClient`
```python
class QuicClient:
    """Wrapper for s2n-quic Client"""
    @staticmethod
    async def create(cert_path: str, bind_addr: str = "0.0.0.0:0") -> QuicClient: ...
```

**Challenge**: `s2n-quic::Client` is a complex type. Need to wrap it carefully.

### 3. RpcServer Extensions

Add to existing `PyRpcServer`:

```python
class RpcServer:
    # ... existing methods ...

    async def enable_cluster(
        self,
        config: ClusterConfig,
        seeds: List[str],  # List of "ip:port" addresses
        quic_client: QuicClient,
    ) -> None:
        """Enable SWIM cluster functionality"""
        ...

    async def cluster(self) -> Optional[Cluster]:
        """Get cluster handle if enabled"""
        ...
```

### 4. Cluster Membership Class

#### `PyCluster`
```python
class Cluster:
    """Handle to cluster membership"""

    async def update_tags(self, tags: List[Tuple[str, str]]) -> None:
        """Update node tags (e.g., role=worker, label=python-worker)"""
        ...

    async def update_tag(self, key: str, value: str) -> None:
        """Update a single tag"""
        ...

    def subscribe(self) -> ClusterEventReceiver:
        """Subscribe to cluster events"""
        ...

    async def stop_heartbeats(self) -> None:
        """Stop sending SWIM ACKs (for testing failure scenarios)"""
        ...

    async def resume_heartbeats(self) -> None:
        """Resume sending SWIM ACKs"""
        ...
```

**Rust mapping**: `Arc<cluster::ClusterMembership>`

### 5. Event System

#### `PyClusterEvent`
```python
@dataclass
class ClusterNode:
    id: str
    addr: str
    tags: Dict[str, str]

class ClusterEvent(Enum):
    NODE_JOINED = "NodeJoined"
    NODE_LEFT = "NodeLeft"
    NODE_FAILED = "NodeFailed"
    NODE_RECOVERED = "NodeRecovered"
    TAG_UPDATED = "TagUpdated"
```

#### `PyClusterEventReceiver`
```python
class ClusterEventReceiver:
    """Async iterator for cluster events"""

    async def recv(self) -> Optional[Tuple[str, ClusterNode]]:
        """Receive next event (event_type, node)"""
        ...

    def __aiter__(self):
        return self

    async def __anext__(self) -> Tuple[str, ClusterNode]:
        event = await self.recv()
        if event is None:
            raise StopAsyncIteration
        return event
```

**Rust mapping**: `cluster::ClusterEventReceiver`

## Implementation Challenges

### 1. **s2n-quic Client Wrapping** (HIGH COMPLEXITY)
- `s2n-quic::Client` is not a simple type
- Requires TLS provider setup
- Need to manage client lifecycle

### 2. **Arc and Thread Safety** (MEDIUM COMPLEXITY)
- `ClusterMembership` is `Arc<...>` in Rust
- Need to properly wrap with PyO3's `Arc` handling

### 3. **Async Channel Bridging** (MEDIUM COMPLEXITY)
- `ClusterEventReceiver` uses Tokio's `broadcast::Receiver`
- Need to bridge Tokio async with Python async

### 4. **Error Handling** (LOW COMPLEXITY)
- Convert `ClusterError` to Python exceptions
- Handle `Option<T>` returns properly

## Estimated Effort

| Component | Complexity | Estimated Lines of Code |
|-----------|------------|-------------------------|
| Config classes | Low | ~150 |
| QuicClient wrapper | High | ~100-150 |
| RpcServer extensions | Medium | ~80-100 |
| Cluster class | Medium | ~100-150 |
| Event system | Medium | ~80-100 |
| **Total** | | **~500-650 lines** |

**Time estimate**: 4-6 hours for an experienced Rust+PyO3 developer

## Alternative: Simplified Approach

Instead of full SWIM integration, implement **manual registration**:

1. Generate Python bindings for `DirectorRegistry` service
2. Python worker calls `register_worker(addr, label, tags)` on startup
3. Director maintains manual registry
4. **Pros**: Much simpler (~50 lines of code)
5. **Cons**: No SWIM gossip, no automatic failure detection

## Recommendation

Given the complexity, I recommend:

1. **Short term**: Document the limitation, provide standalone Python worker example
2. **Medium term**: Implement simplified manual registration (Option 3 from earlier)
3. **Long term**: Implement full SWIM integration when there's sustained demand

The full SWIM integration is valuable but represents significant engineering effort. The Python worker example is already functional for demonstrating Python RPC capabilities.

## Example Usage (if implemented)

```python
# Create QUIC client
quic_client = await _rpcnet.QuicClient.create(
    cert_path="certs/test_cert.pem",
    bind_addr="0.0.0.0:0"
)

# Create cluster config
cluster_config = _rpcnet.ClusterConfig(
    gossip=_rpcnet.GossipConfig(protocol_period_ms=500),
    health=_rpcnet.HealthCheckConfig(phi_threshold=5.0),
)

# Enable cluster
await server.enable_cluster(
    config=cluster_config,
    seeds=["127.0.0.1:61000"],  # Director address
    quic_client=quic_client,
)

# Get cluster handle
cluster = await server.cluster()

# Update tags
await cluster.update_tags([
    ("role", "worker"),
    ("label", "python-worker"),
    ("gpu", "true"),
])

# Subscribe to events
events = cluster.subscribe()
async for event_type, node in events:
    print(f"Cluster event: {event_type} - {node.id} at {node.addr}")
```
