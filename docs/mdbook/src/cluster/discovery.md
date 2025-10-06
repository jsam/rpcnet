# Automatic Discovery

RpcNet uses the **SWIM (Scalable Weakly-consistent Infection-style Process Group Membership)** protocol for automatic node discovery. This chapter explains how nodes find each other without central coordination or manual registration.

## How Discovery Works

### The Problem

In distributed systems, you need to know:
- Which nodes are currently alive?
- Which nodes just joined?
- Which nodes have failed or left?

Traditional solutions have limitations:
- **Centralized registry**: Single point of failure
- **Broadcast**: Doesn't scale (O(N²) messages)
- **Heartbeats**: Network overhead grows with cluster size

### The SWIM Solution

SWIM provides **scalable membership** with constant overhead per node:

```
┌─────────────────────────────────────────────────────┐
│  Node A discovers new nodes through gossip          │
│  without contacting every node in the cluster       │
└─────────────────────────────────────────────────────┘

     Node A                    Node B                    Node C
       │                         │                         │
       │   1. Ping (health)      │                         │
       ├────────────────────────►│                         │
       │                         │                         │
       │   2. Ack + Gossip       │                         │
       │◄────────────────────────┤                         │
       │   (includes info        │                         │
       │    about Node C)        │                         │
       │                         │                         │
       │   3. Now A knows C      │                         │
       │   exists without        │                         │
       │   direct contact!       │                         │
       │                         │                         │
       └─────────────┬───────────┴─────────────────────────┘
                     │
              Information spreads
              exponentially fast
```

## SWIM Protocol Basics

### 1. Gossip-Based Communication

Nodes periodically exchange information with random peers:

```rust
// Simplified gossip cycle (every 1 second by default)
loop {
    // Pick random node
    let peer = select_random_node();
    
    // Send health check + gossip payload
    let gossip = GossipMessage {
        sender: my_node_id,
        members: my_known_members.clone(),
        incarnation: my_incarnation,
    };
    peer.ping(gossip).await?;
    
    // Receive ack + peer's gossip
    let ack = receive_ack().await?;
    merge_member_information(ack.members);
    
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

**Key properties**:
- Constant overhead per node: O(1) messages per cycle
- Information spreads exponentially: O(log N) time
- No single point of failure
- Works with network partitions

### 2. Three Node States

SWIM tracks nodes in three states:

```rust
pub enum NodeState {
    Alive,      // Node is healthy and responding
    Suspect,    // Node might be failed (under investigation)
    Failed,     // Node confirmed failed
}
```

**State transitions**:

```
         ┌──────────────────────────────────────┐
         │                                      │
         │  Join cluster                        │  Gossip confirms alive
         │                                      │
    ┌────▼─────┐  No response after 3 pings  ┌─▼──────┐
    │  Alive   ├───────────────────────────►  │Suspect │
    └────┬─────┘                              └───┬────┘
         │                                        │
         │  Voluntary leave                       │  Confirmed by multiple nodes
         │                                        │  or timeout
         │                                    ┌───▼────┐
         └───────────────────────────────────►│ Failed │
                                              └────────┘
```

### 3. Failure Detection Protocol

SWIM uses **indirect probing** to avoid false positives:

**Direct Probe** (normal case):
```
Node A                  Node B
  │                       │
  │  1. Ping              │
  ├──────────────────────►│
  │                       │
  │  2. Ack               │
  │◄──────────────────────┤
  │                       │
  │  B is alive ✓         │
```

**Indirect Probe** (when direct fails):
```
Node A                  Node C                  Node B
  │                       │                       │
  │  1. Ping (timeout)    │                       │
  ├─────────────────────X─┤                       │
  │                       │                       │
  │  2. Ask C to probe B  │                       │
  ├──────────────────────►│                       │
  │                       │  3. Ping              │
  │                       ├──────────────────────►│
  │                       │                       │
  │                       │  4. Ack               │
  │                       │◄──────────────────────┤
  │  5. B is alive via C  │                       │
  │◄──────────────────────┤                       │
  │                       │                       │
  │  B is alive ✓         │                       │
```

This prevents false positives from temporary network issues.

## RpcNet Implementation

### Joining a Cluster

When a node starts, it joins by contacting one or more **seed nodes**:

```rust
use rpcnet::cluster::{ClusterMembership, ClusterConfig};

// Create cluster membership
let cluster_config = ClusterConfig::default()
    .with_bind_addr("0.0.0.0:7946".parse()?);

let cluster = ClusterMembership::new(cluster_config).await?;

// Join via seed nodes (directors, known workers, etc.)
let seeds = vec![
    "director.example.com:7946".parse()?,
    "worker-1.example.com:7946".parse()?,
];

cluster.join(seeds).await?;
```

**What happens during join**:

1. **Contact seed nodes**: Node sends join request to all seeds
2. **Receive member list**: Seed responds with known cluster members
3. **Merge member info**: Node learns about entire cluster
4. **Start gossip**: Node begins exchanging info with all members
5. **Spread join event**: Other nodes learn about new member via gossip

**Time to full discovery**: ~O(log N) gossip cycles (typically 2-5 seconds)

### Tagging Nodes

Nodes can advertise capabilities via **tags**:

```rust
// Tag worker with role and capabilities
cluster.set_tag("role", "worker");
cluster.set_tag("label", "worker-gpu-1");
cluster.set_tag("gpu", "true");
cluster.set_tag("zone", "us-west-2a");
cluster.set_tag("memory", "64GB");
```

**Tags are gossiped** to all nodes, enabling:
- Service discovery (find all nodes with `role=worker`)
- Capability-based routing (find nodes with `gpu=true`)
- Zone-aware load balancing (prefer nodes in `zone=us-west-2a`)

### Subscribing to Events

Monitor cluster changes in real-time:

```rust
use rpcnet::cluster::ClusterEvent;

let mut events = cluster.subscribe();

while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeJoined(node) => {
            println!("New node: {} at {}", node.id, node.addr);
            println!("Tags: {:?}", node.tags);
        }
        ClusterEvent::NodeLeft(node) => {
            println!("Node left gracefully: {}", node.id);
        }
        ClusterEvent::NodeFailed(node) => {
            println!("Node failed: {}", node.id);
            // Take action: remove from pool, alert monitoring, etc.
        }
    }
}
```

## Gossip Internals

### Gossip Message Structure

Each gossip message contains:

```rust
struct GossipMessage {
    // Sender identification
    sender_id: Uuid,
    sender_addr: SocketAddr,
    incarnation: u64,  // Anti-entropy counter
    
    // Member information
    members: Vec<MemberInfo>,
    
    // Piggyback information
    events: Vec<ClusterEvent>,
}

struct MemberInfo {
    id: Uuid,
    addr: SocketAddr,
    state: NodeState,
    incarnation: u64,
    tags: HashMap<String, String>,
    last_seen: SystemTime,
}
```

### Gossip Cycle

**Every gossip interval** (default: 1 second):

1. **Select target**: Pick random node from member list
2. **Prepare message**: Collect recent events and member updates
3. **Send ping**: UDP datagram with gossip payload
4. **Wait for ack**: Timeout after 500ms (configurable)
5. **Merge information**: Update local member list with received data
6. **Detect failures**: Check for nodes that haven't responded

### Information Spread Speed

With **N nodes** and **gossip interval T**:

- **1 node** knows: T seconds (initial)
- **2 nodes** know: 2T seconds (1st gossip)
- **4 nodes** know: 3T seconds (2nd gossip)
- **8 nodes** know: 4T seconds (3rd gossip)
- **N nodes** know: (log₂ N) × T seconds

**Example**: 1000-node cluster, 1-second interval:
- Full propagation: ~10 seconds (log₂ 1000 ≈ 10)

## Advanced Features

### Incarnation Numbers

Each node maintains an **incarnation counter** to handle:

**Problem**: Node A suspects Node B is failed, but B is actually alive.

**Solution**: B increments its incarnation number and gossips "I'm alive with incarnation N+1". This overrides stale failure suspicion.

```rust
// Node B refutes failure suspicion
if cluster.is_suspected() {
    cluster.increment_incarnation();
    cluster.broadcast_alive();
}
```

### Anti-Entropy

Periodically, nodes perform **full state synchronization** to:
- Fix inconsistencies from packet loss
- Recover from network partitions
- Ensure eventual consistency

```rust
// Every 10 gossip cycles, do full sync with random node
if cycle_count % 10 == 0 {
    let peer = select_random_node();
    let full_state = get_all_members();
    peer.sync(full_state).await?;
}
```

### Partition Detection

SWIM can detect **network partitions**:

```
Before partition:            After partition:
     Cluster                     Cluster A  |  Cluster B
        │                            │      |      │
  ┌─────┼─────┐                ┌─────┼─────┐|┌─────┼─────┐
  A     B     C                A     B      ||     C     D
  │     │     │                │     │      ||     │     │
  └─────┼─────┘                └─────┘      |└─────┘     
        D                                   |
                                         SPLIT!
```

**Detection**: Nodes in partition A can't reach nodes in partition B after multiple indirect probes.

**Handling**:
- Each partition continues operating independently
- When partition heals, gossip merges the views
- Application must handle split-brain scenarios

## Configuration

### Tuning Gossip Parameters

```rust
use rpcnet::cluster::ClusterConfig;
use std::time::Duration;

let config = ClusterConfig::default()
    .with_bind_addr("0.0.0.0:7946".parse()?)
    .with_gossip_interval(Duration::from_secs(1))      // How often to gossip
    .with_probe_timeout(Duration::from_millis(500))    // Ping timeout
    .with_indirect_probes(3)                           // How many indirect probes
    .with_suspicion_timeout(Duration::from_secs(5))    // Suspect → Failed timeout
    .with_gossip_fanout(3);                            // How many nodes to gossip to

cluster = ClusterMembership::new(config).await?;
```

### Tuning Guidelines

**Small clusters** (< 10 nodes):
- Longer intervals (2-3 seconds)
- Faster timeouts (200ms)
- Lower fanout (1-2 nodes)

**Medium clusters** (10-100 nodes):
- Default settings (1 second, 500ms, 3 fanout)

**Large clusters** (100-1000 nodes):
- Shorter intervals (500ms)
- More indirect probes (5+)
- Higher fanout (5-7 nodes)

**Very large clusters** (1000+ nodes):
- Consider hierarchical clustering
- Adjust suspicion timeout upward
- Use regional seed nodes

## Failure Scenarios

### Temporary Network Glitch

```
Node A pings B → timeout (network glitch)
Node A → Suspect B
Node A asks C to probe B
Node C → B responds ✓
Node A → B is Alive (false alarm avoided)
```

**Result**: No false positive due to indirect probing.

### Actual Node Failure

```
Node A pings B → timeout
Node A → Suspect B
Node A asks C, D, E to probe B → all timeout
Suspicion timeout expires (5 seconds)
Node A → B is Failed
Gossip spreads: B failed
All nodes remove B from active pool
```

**Result**: B marked failed within ~6 seconds (1s ping + 5s suspicion).

### Network Partition

```
Partition occurs: {A, B} | {C, D}

In partition {A, B}:
- A and B communicate normally
- C and D marked as Failed

In partition {C, D}:
- C and D communicate normally
- A and B marked as Failed

Partition heals:
- Gossip exchanges full state
- All nodes marked Alive again
- Incarnation numbers resolve conflicts
```

**Result**: Both partitions continue operating; merge when healed.

## Best Practices

### 1. Use Multiple Seed Nodes

```rust
// ✅ Good: Multiple seeds for reliability
let seeds = vec![
    "seed-1.cluster.local:7946".parse()?,
    "seed-2.cluster.local:7946".parse()?,
    "seed-3.cluster.local:7946".parse()?,
];

// ❌ Bad: Single seed (single point of failure)
let seeds = vec!["seed-1.cluster.local:7946".parse()?];
```

### 2. Monitor Cluster Events

```rust
// Log all cluster changes for debugging
tokio::spawn(async move {
    let mut events = cluster.subscribe();
    while let Some(event) = events.recv().await {
        log::info!("Cluster event: {:?}", event);
        metrics.record_cluster_event(&event);
    }
});
```

### 3. Tag Nodes with Rich Metadata

```rust
// Provide detailed tags for routing decisions
cluster.set_tag("role", "worker");
cluster.set_tag("version", env!("CARGO_PKG_VERSION"));
cluster.set_tag("zone", get_availability_zone());
cluster.set_tag("instance_type", "m5.xlarge");
cluster.set_tag("capabilities", "gpu,video-encode");
```

### 4. Handle Partition Detection

```rust
// Detect partitions and alert
let mut events = cluster.subscribe();
while let Some(event) = events.recv().await {
    if let ClusterEvent::PartitionDetected = event {
        alert_ops_team("Network partition detected!");
        enable_read_only_mode(); // Prevent split-brain writes
    }
}
```

### 5. Graceful Shutdown

```rust
// Leave cluster gracefully when shutting down
cluster.leave().await?;

// This tells other nodes "I'm leaving intentionally"
// rather than waiting for failure detection timeout
```

## Comparison to Other Protocols

| Feature | SWIM (RpcNet) | Raft | Consul | Kubernetes |
|---------|---------------|------|--------|------------|
| **Consistency** | Eventual | Strong | Strong | Eventual |
| **Failure Detection** | Phi Accrual | Leader heartbeat | Gossip | kubelet heartbeat |
| **Scalability** | 1000+ nodes | ~10 nodes | 100s of nodes | 1000s of nodes |
| **Partition Handling** | Both sides live | Majority only | Both sides live | Both sides live |
| **Network Overhead** | O(1) per node | O(N) from leader | O(1) per node | O(1) per node |
| **Setup Complexity** | Low | Medium | Medium | High |

**When to use SWIM**:
- Large clusters (100+ nodes)
- Partition tolerance required
- Eventual consistency acceptable
- Decentralized architecture preferred

**When NOT to use SWIM**:
- Strong consistency required → Use Raft
- Small clusters (< 5 nodes) → Direct RPC simpler
- Centralized control desired → Use coordinator pattern

## Troubleshooting

### Nodes Not Discovering

**Symptom**: Workers join but director doesn't see them.

**Debug**:
```rust
// Enable debug logging
RUST_LOG=rpcnet::cluster=debug cargo run

// Check what nodes are known
let members = cluster.members().await;
println!("Known members: {:?}", members);
```

**Common causes**:
- Firewall blocking UDP gossip port
- Wrong seed node address
- Network partition

### Slow Propagation

**Symptom**: Takes 30+ seconds for nodes to discover each other.

**Debug**:
```rust
// Check gossip interval
let config = ClusterConfig::default()
    .with_gossip_interval(Duration::from_millis(500)); // Faster
```

**Common causes**:
- Gossip interval too long
- High packet loss
- Too few gossip fanout targets

### False Failure Detection

**Symptom**: Nodes marked failed but they're actually alive.

**Debug**:
```rust
// Increase timeouts
let config = ClusterConfig::default()
    .with_probe_timeout(Duration::from_secs(1))    // More lenient
    .with_suspicion_timeout(Duration::from_secs(10));
```

**Common causes**:
- Network latency spikes
- Node overloaded (GC pauses)
- Timeout too aggressive

## Next Steps

- **[Load Balancing](load-balancing.md)** - Use discovered nodes for routing
- **[Health Checking](health.md)** - Understand Phi Accrual algorithm
- **[Failures](failures.md)** - Handle partitions and split-brain scenarios

## References

- [SWIM Paper (Cornell)](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf) - Original SWIM protocol
- [Phi Accrual Paper](https://citeseerx.ist.psu.edu/document?repid=rep1&type=pdf&doi=babf246cf6753ad12ce97ae47e64c9d4ff85c6f7) - Advanced failure detection
- [Gossip Protocols Overview](https://en.wikipedia.org/wiki/Gossip_protocol) - General gossip concepts
