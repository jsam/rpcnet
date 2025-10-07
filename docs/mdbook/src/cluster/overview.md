# Cluster Overview

RpcNet provides built-in support for building distributed RPC clusters with automatic service discovery, intelligent load balancing, and robust failure detection. This chapter introduces the core concepts and components of RpcNet's cluster architecture.

## What is a Cluster?

A **cluster** in RpcNet is a group of interconnected nodes that work together to provide distributed RPC services. Nodes automatically discover each other, share information about their state, and coordinate to handle client requests efficiently.

### Key Benefits

**Automatic Discovery** 🔍
- No manual node registration required
- Nodes join and leave seamlessly
- Gossip protocol spreads information automatically

**Intelligent Load Balancing** ⚖️
- Multiple strategies (Round Robin, Random, Least Connections)
- Tracks active connections per node
- Prevents overload on individual nodes

**Robust Failure Detection** 💓
- Phi Accrual failure detection algorithm
- Adapts to network conditions
- Distinguishes between slow and failed nodes

**Tag-Based Routing** 🏷️
- Route requests by node capabilities
- Filter by zone, hardware type, role, etc.
- Enables heterogeneous worker pools

## Architecture Components

RpcNet's cluster architecture consists of several key components that work together:

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                        │
│  (Your RPC handlers, business logic)                         │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                    ClusterClient                             │
│  - High-level API for cluster operations                    │
│  - Load-balanced request routing                            │
│  - Efficient request routing                               │
└────────────────────────┬────────────────────────────────────┘
                         │
        │
┌───────▼─────────┐
│ WorkerRegistry  │
│  - Tracks nodes │
│  - Load balance │
│  - Filter tags  │
└───────┬─────────┘
        │
┌───────▼─────────┐
│  NodeRegistry   │
│  - All nodes    │
│  - Health state │
│  - Metadata     │
└───────┬─────────┘
        │
┌───────▼─────────────────────────────────────────────────────┐
│              ClusterMembership (SWIM)                        │
│  - Gossip protocol for node discovery                       │
│  - Phi Accrual failure detection                            │
│  - Event notifications (NodeJoined/Left/Failed)             │
└──────────────────────────────────────────────────────────────┘
```

### 1. ClusterMembership (SWIM)

The foundation of RpcNet's cluster is the **SWIM (Scalable Weakly-consistent Infection-style Process Group Membership)** protocol. This provides:

- **Gossip-based communication**: Nodes periodically exchange information
- **Failure detection**: Phi Accrual algorithm detects node failures accurately
- **Partition detection**: Identifies network splits and handles them gracefully
- **Event system**: Notifies about node state changes

**Key characteristics**:
- Eventually consistent membership information
- Scales to thousands of nodes
- Low network overhead (UDP-based gossip)
- Handles network partitions and node churn

### 2. NodeRegistry

The **NodeRegistry** maintains a comprehensive view of all nodes in the cluster:

```rust
use rpcnet::cluster::{NodeRegistry, ClusterMembership};

let registry = Arc::new(NodeRegistry::new(cluster));
registry.start().await;

// Get all nodes
let nodes = registry.nodes().await;

// Subscribe to cluster events
let mut events = registry.subscribe();
while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeJoined(node) => println!("Node joined: {}", node.id),
        ClusterEvent::NodeLeft(node) => println!("Node left: {}", node.id),
        ClusterEvent::NodeFailed(node) => println!("Node failed: {}", node.id),
    }
}
```

**Features**:
- Real-time node tracking
- Metadata storage per node
- Event subscription for state changes
- Thread-safe access via `Arc`

### 3. WorkerRegistry

The **WorkerRegistry** extends NodeRegistry to track worker nodes specifically:

```rust
use rpcnet::cluster::{WorkerRegistry, LoadBalancingStrategy};

let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;

// Select a worker (with optional tag filter)
let worker = registry.select_worker(Some("role=worker")).await?;
println!("Selected worker: {} at {}", worker.label, worker.addr);
```

**Features**:
- Filters nodes by tags (e.g., `role=worker`)
- Applies load balancing strategy
- Tracks active connections per worker
- Automatic removal of failed workers

### 4. ClusterClient

The **ClusterClient** provides a high-level API that combines all components:

```rust
use rpcnet::cluster::{ClusterClient, ClusterClientConfig};

let client = Arc::new(ClusterClient::new(registry, config));

// Call any worker matching the filter
let result = client.call_worker("compute", request, Some("role=worker")).await?;
```

**Features**:
- Automatic worker selection
- Load-balanced request routing
- Efficient connection management
- Retry logic for failed requests

## When to Use Clusters

RpcNet clusters are ideal for scenarios where you need:

### ✅ Good Use Cases

**Distributed Workload Processing**
- Multiple workers processing tasks in parallel
- Automatic load distribution across workers
- Example: Video transcoding farm, data processing pipeline

**High Availability Services**
- Services that must tolerate node failures
- Automatic failover to healthy nodes
- Example: API gateway, microservices mesh

**Dynamic Scaling**
- Add/remove nodes based on load
- Automatic discovery of new capacity
- Example: Auto-scaling worker pools, elastic compute clusters

**Heterogeneous Worker Pools**
- Different node types (GPU vs CPU, different zones)
- Tag-based routing to appropriate nodes
- Example: ML inference with GPU/CPU workers, multi-region deployments

### ❌ When NOT to Use Clusters

**Single Node Deployments**
- If you only have one server, use direct RPC instead
- Cluster overhead isn't justified

**Strict Consistency Requirements**
- SWIM provides eventual consistency
- Not suitable for strong consistency needs (use consensus protocols like Raft)

**Low-Latency Single-Hop**
- Direct RPC is faster for single client-server communication
- Cluster adds minimal overhead, but every bit counts for ultra-low latency

## Cluster Modes

RpcNet supports different cluster deployment patterns:

### 1. Coordinator-Worker Pattern

One or more coordinator nodes route requests to worker nodes:

```
         ┌──────────────┐
         │  Coordinator │
         │  (Director)  │
         └──────┬───────┘
                │
    ┌───────────┼───────────┐
    │           │           │
┌───▼───┐   ┌──▼────┐   ┌──▼────┐
│Worker │   │Worker │   │Worker │
└───────┘   └───────┘   └───────┘
```

**Use when**:
- Clients don't need to track worker pool
- Centralized routing and monitoring
- Example: Load balancer + worker pool

### 2. Peer-to-Peer Pattern

All nodes are equal and can route to each other:

```
┌──────┐     ┌──────┐
│ Node ├─────┤ Node │
└───┬──┘     └──┬───┘
    │           │
    └─────┬─────┘
      ┌───▼───┐
      │ Node  │
      └───────┘
```

**Use when**:
- No single point of coordination needed
- Nodes serve both as clients and servers
- Example: Distributed cache, gossip-based database

### 3. Hierarchical Pattern

Multiple layers with different roles:

```
       ┌────────┐
       │ Master │
       └───┬────┘
           │
    ┌──────┼──────┐
┌───▼───┐     ┌───▼───┐
│Region │     │Region │
│Leader │     │Leader │
└───┬───┘     └───┬───┘
    │             │
┌───▼───┐     ┌───▼───┐
│Worker │     │Worker │
└───────┘     └───────┘
```

**Use when**:
- Multi-region deployments
- Different node tiers (leaders, workers, storage)
- Example: Global CDN, multi-tenant systems

## Performance Characteristics

RpcNet clusters maintain high performance while providing distributed coordination:

### Throughput

- **172K+ requests/second** in benchmarks
- Minimal overhead compared to direct RPC
- Scales linearly with number of workers

### Latency

- **< 0.1ms** additional latency for load balancing
- Efficient connection handling reduces overhead
- QUIC's 0-RTT mode for warm connections

### Scalability

- Tested with **1000+ nodes** in gossip cluster
- Sub-linear gossip overhead (O(log N) per node)
- Configurable gossip intervals for tuning

### Resource Usage

- **Low memory**: ~10KB per tracked node
- **Low CPU**: < 1% for gossip maintenance
- **Low network**: ~1KB/s per node for gossip

## Next Steps

Now that you understand the cluster architecture, you can:

1. **[Follow the Tutorial](tutorial.md)** - Build your first cluster step-by-step
2. **[Learn About Discovery](discovery.md)** - Deep dive into SWIM gossip protocol
3. **[Explore Load Balancing](load-balancing.md)** - Choose the right strategy
4. **[Understand Health Checking](health.md)** - How Phi Accrual works
6. **[Handle Failures](failures.md)** - Partition detection and recovery

Or jump directly to the **[Cluster Example](../cluster-example.md)** to see a complete working system.
