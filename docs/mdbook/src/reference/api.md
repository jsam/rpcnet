# API Reference

Quick reference for RpcNet's most commonly used APIs. For complete documentation, see the [API docs](https://docs.rs/rpcnet).

## Core Types

### Server

Creates and manages RPC servers.

```rust
use rpcnet::{Server, ServerConfig};

// Create server
let config = ServerConfig::builder()
    .with_cert_and_key(cert, key)?
    .build();
let mut server = Server::new(config);

// Register services
server.register_service(Arc::new(MyService));

// Bind and run
server.bind("0.0.0.0:8080").await?;
server.run().await?;
```

**Key methods**:
- `new(config)` - Create server with configuration
- `register_service(service)` - Register RPC service handler
- `bind(addr)` - Bind to address
- `enable_cluster(config)` - Enable cluster features
- `run()` - Start server (blocks until shutdown)
- `shutdown()` - Gracefully shut down server

### Client

Connects to RPC servers and makes requests.

```rust
use rpcnet::{Client, ClientConfig};

// Create client
let config = ClientConfig::builder()
    .with_server_cert(cert)?
    .build();

// Connect
let client = MyServiceClient::connect("server.example.com:8080", config).await?;

// Make request
let response = client.my_method(args).await?;
```

**Key methods**:
- `connect(addr, config)` - Connect to server
- Generated methods per RPC trait
- Auto-reconnect on connection loss

## Cluster APIs

### ClusterMembership

Manages node membership via SWIM gossip protocol.

```rust
use rpcnet::cluster::ClusterMembership;

// Create cluster
let config = ClusterConfig::default()
    .with_bind_addr("0.0.0.0:7946".parse()?);
let cluster = ClusterMembership::new(config).await?;

// Join via seed nodes
cluster.join(vec!["seed.example.com:7946".parse()?]).await?;

// Tag node
cluster.set_tag("role", "worker");

// Subscribe to events
let mut events = cluster.subscribe();
while let Some(event) = events.recv().await {
    // Handle cluster events
}
```

**Key methods**:
- `new(config)` - Create cluster membership
- `join(seeds)` - Join cluster via seed nodes
- `leave()` - Gracefully leave cluster
- `set_tag(key, value)` - Set metadata tag
- `get_tag(key)` - Get metadata tag
- `nodes()` - Get all cluster nodes
- `subscribe()` - Subscribe to cluster events
- `local_node_id()` - Get local node ID

### WorkerRegistry

Tracks worker nodes with load balancing.

```rust
use rpcnet::cluster::{WorkerRegistry, LoadBalancingStrategy};

// Create registry
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));

// Start monitoring
registry.start().await;

// Select worker
let worker = registry.select_worker(Some("role=worker")).await?;
println!("Selected: {} at {}", worker.label, worker.addr);

// Get all workers
let workers = registry.workers().await;
```

**Key methods**:
- `new(cluster, strategy)` - Create registry
- `start()` - Start monitoring cluster events
- `select_worker(filter)` - Select worker by tag filter
- `workers()` - Get all workers
- `worker_count()` - Get number of workers
- `subscribe()` - Subscribe to registry events

### NodeRegistry

Tracks all cluster nodes.

```rust
use rpcnet::cluster::NodeRegistry;

// Create registry
let registry = Arc::new(NodeRegistry::new(cluster));
registry.start().await;

// Get all nodes
let nodes = registry.nodes().await;

// Filter by tag
let directors = nodes.iter()
    .filter(|n| n.tags.get("role") == Some(&"director".to_string()))
    .collect::<Vec<_>>();
```

**Key methods**:
- `new(cluster)` - Create node registry
- `start()` - Start monitoring cluster
- `nodes()` - Get all nodes
- `node_count()` - Count nodes
- `subscribe()` - Subscribe to events

### ClusterClient

High-level API for calling workers.

```rust
use rpcnet::cluster::{ClusterClient, ClusterClientConfig};

// Create client
let config = ClusterClientConfig::default();
let client = Arc::new(ClusterClient::new(registry, config));

// Call any worker
let result = client.call_worker("compute", request, Some("role=worker")).await?;
```

**Key methods**:
- `new(registry, config)` - Create cluster client
- `call_worker(method, data, filter)` - Call any worker matching filter

## Configuration

### ServerConfig

```rust
use rpcnet::ServerConfig;

let config = ServerConfig::builder()
    .with_cert_and_key(cert, key)?           // TLS certificate and key
    .with_ca_cert(ca)?                        // CA certificate for client verification
    .with_max_concurrent_streams(100)?       // Max concurrent QUIC streams
    .with_max_idle_timeout(Duration::from_secs(30))? // Idle timeout
    .build();
```

### ClientConfig

```rust
use rpcnet::ClientConfig;

let config = ClientConfig::builder()
    .with_server_cert(cert)?                 // Server certificate
    .with_ca_cert(ca)?                       // CA certificate
    .with_connect_timeout(Duration::from_secs(5))? // Connection timeout
    .build();
```

### ClusterConfig

```rust
use rpcnet::cluster::ClusterConfig;

let config = ClusterConfig::default()
    .with_bind_addr("0.0.0.0:7946".parse()?)
    .with_gossip_interval(Duration::from_secs(1))
    .with_health_check_interval(Duration::from_secs(2))
    .with_phi_threshold(8.0);
```


## Code Generation

### RPC Trait Definition

```rust
use rpcnet::prelude::*;

#[rpc_trait]
pub trait MyService {
    async fn my_method(&self, arg1: String, arg2: i32) -> Result<Response>;
    async fn streaming(&self, request: Request) -> impl Stream<Item = Result<Chunk>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub data: Vec<u8>,
}
```

### Generate Code

```bash
rpcnet-gen --input my_service.rpc.rs --output src/generated
```

### Use Generated Code

```rust
mod generated;
use generated::my_service::*;

// Server side
#[rpc_impl]
impl MyService for Handler {
    async fn my_method(&self, arg1: String, arg2: i32) -> Result<Response> {
        // Implementation
    }
}

// Client side
let client = MyServiceClient::connect(addr, config).await?;
let response = client.my_method("test".to_string(), 42).await?;
```

## Streaming

### Server-Side Streaming

```rust
#[rpc_trait]
pub trait StreamService {
    async fn stream_data(&self, count: usize) -> impl Stream<Item = Result<Data>>;
}

#[rpc_impl]
impl StreamService for Handler {
    async fn stream_data(&self, count: usize) -> impl Stream<Item = Result<Data>> {
        futures::stream::iter(0..count).map(|i| {
            Ok(Data { value: i })
        })
    }
}
```

### Client-Side Streaming

```rust
#[rpc_trait]
pub trait UploadService {
    async fn upload(&self, stream: impl Stream<Item = Chunk>) -> Result<Summary>;
}

// Client usage
let chunks = futures::stream::iter(vec![chunk1, chunk2, chunk3]);
let summary = client.upload(chunks).await?;
```

### Bidirectional Streaming

```rust
#[rpc_trait]
pub trait ChatService {
    async fn chat(&self, stream: impl Stream<Item = Message>) 
        -> impl Stream<Item = Result<Message>>;
}
```

## Load Balancing Strategies

```rust
use rpcnet::cluster::LoadBalancingStrategy;

// Round Robin - even distribution
LoadBalancingStrategy::RoundRobin

// Random - stateless selection
LoadBalancingStrategy::Random

// Least Connections - pick least loaded (recommended)
LoadBalancingStrategy::LeastConnections
```

## Cluster Events

```rust
use rpcnet::cluster::ClusterEvent;

let mut events = cluster.subscribe();
while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeJoined(node) => {
            println!("Node {} joined at {}", node.id, node.addr);
        }
        ClusterEvent::NodeLeft(node) => {
            println!("Node {} left", node.id);
        }
        ClusterEvent::NodeFailed(node) => {
            println!("Node {} failed", node.id);
        }
        ClusterEvent::NodeUpdated(node) => {
            println!("Node {} updated", node.id);
        }
        ClusterEvent::PartitionDetected(minority, majority) => {
            println!("Partition detected!");
        }
    }
}
```

## Error Handling

```rust
use rpcnet::{Error, ErrorKind};

match client.call("method", args).await {
    Ok(response) => {
        // Handle success
    }
    Err(e) => {
        match e.kind() {
            ErrorKind::ConnectionFailed => {
                // Connection issue, retry with different worker
            }
            ErrorKind::Timeout => {
                // Request timed out
            }
            ErrorKind::SerializationError => {
                // Data serialization failed
            }
            ErrorKind::ApplicationError => {
                // Application-level error from handler
            }
            _ => {
                // Other errors
            }
        }
    }
}
```

## Common Patterns

### Health Check Endpoint

```rust
#[rpc_trait]
pub trait HealthService {
    async fn health(&self) -> Result<HealthStatus>;
}

#[derive(Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub version: String,
    pub uptime_secs: u64,
}
```

### Graceful Shutdown

```rust
use tokio::signal;

async fn run(mut server: Server, cluster: Arc<ClusterMembership>) -> Result<()> {
    let server_task = tokio::spawn(async move { server.run().await });
    
    signal::ctrl_c().await?;
    
    // Leave cluster gracefully
    cluster.leave().await?;
    
    // Wait for in-flight requests
    server.shutdown().await?;
    
    Ok(())
}
```

### Connection Retry

```rust
async fn call_with_retry<T>(
    f: impl Fn() -> Pin<Box<dyn Future<Output = Result<T>>>>,
    max_retries: usize,
) -> Result<T> {
    for attempt in 0..max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                tokio::time::sleep(Duration::from_millis(100 * 2_u64.pow(attempt as u32))).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

## Environment Variables

Common environment variables used in examples:

```bash
# Director
DIRECTOR_ADDR=127.0.0.1:61000
RUST_LOG=info

# Worker
WORKER_LABEL=worker-1
WORKER_ADDR=127.0.0.1:62001
DIRECTOR_ADDR=127.0.0.1:61000

# Client
CLIENT_ID=client-1

# Logging
RUST_LOG=rpcnet=debug,my_app=info
```

## Feature Flags

```toml
[dependencies]
rpcnet = { version = "0.2", features = ["cluster", "metrics"] }
```

Available features:
- `cluster` - Enable cluster features (WorkerRegistry, ClusterClient, etc.)
- `metrics` - Enable Prometheus metrics
- `codegen` - Enable code generation support (always included in v0.2+)

## Quick Examples

### Simple RPC Server

```rust
use rpcnet::prelude::*;

#[rpc_trait]
pub trait Echo {
    async fn echo(&self, msg: String) -> Result<String>;
}

#[rpc_impl]
impl Echo for Handler {
    async fn echo(&self, msg: String) -> Result<String> {
        Ok(msg)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = ServerConfig::builder()
        .with_cert_and_key(cert, key)?
        .build();
    
    let mut server = Server::new(config);
    server.register_service(Arc::new(Handler));
    server.bind("0.0.0.0:8080").await?;
    server.run().await?;
    Ok(())
}
```

### Simple RPC Client

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = ClientConfig::builder()
        .with_server_cert(cert)?
        .build();
    
    let client = EchoClient::connect("localhost:8080", config).await?;
    let response = client.echo("Hello!".to_string()).await?;
    println!("Response: {}", response);
    Ok(())
}
```

## Next Steps

- **[Examples](examples.md)** - Complete example programs
- **[Cluster Tutorial](../cluster/tutorial.md)** - Build a cluster
- **[API Documentation](https://docs.rs/rpcnet)** - Full API docs
