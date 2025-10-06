# Concepts

This chapter collects the fundamental ideas behind RpcNet: the runtime building
blocks, how servers and clients are constructed, and the streaming patterns that
sit on top of QUIC.

## Runtime Building Blocks

### Configuration (`RpcConfig`)

`RpcConfig` encapsulates the TLS artifacts, socket bindings, and optional
keep-alive settings shared by clients and servers.

```rust
use rpcnet::RpcConfig;

let config = RpcConfig::new("certs/server.pem", "127.0.0.1:0")
    .with_key_path("certs/server-key.pem")
    .with_server_name("localhost")
    .with_keep_alive_interval(std::time::Duration::from_secs(30));
```

Keep-alive is optional; when enabled the interval is mirrored on both ends of
the connection so heartbeats stay in sync.

### Error Handling (`RpcError`)

`RpcError` differentiates between connection, stream, TLS, configuration, IO,
and serialization failures so callers can branch on the exact condition instead
of parsing strings:

```rust
match client.call("ping", vec![]).await {
    Ok(bytes) => println!("pong: {}", String::from_utf8_lossy(&bytes)),
    Err(rpcnet::RpcError::Timeout) => eprintln!("server took too long"),
    Err(other) => eprintln!("unhandled rpc error: {other}")
}
```

### Serialization Strategy

Requests and responses travel as `Vec<u8>`. Examples use `bincode` for compact
frames, but any serialization format can be layered on top.

### Concurrency Model

Each accepted QUIC connection runs inside its own Tokio task. Within that
connection, every RPC request is processed on another task so long-running
handlers never block unrelated work. Clients open a fresh bidirectional stream
per call while sharing a single connection behind an `Arc` + `RwLock`.

## Server Essentials

### Creating the Server

```rust
use rpcnet::{RpcServer, RpcConfig};

let config = RpcConfig::new("certs/server.pem", "127.0.0.1:8080")
    .with_key_path("certs/server-key.pem")
    .with_server_name("localhost");
let mut server = RpcServer::new(config);
```

Binding to port `0` lets the OS allocate a free port. Once `bind()` succeeds the
chosen address is stored on `server.socket_addr`.

### Registering Unary Handlers

Handlers receive raw `Vec<u8>` payloads and return serialized responses. The
closure executes inside a Tokio task, so async IO is allowed.

```rust
use rpcnet::{RpcError, RpcServer};

server.register("add", |params| async move {
    let (a, b): (i32, i32) = bincode::deserialize(&params)
        .map_err(RpcError::SerializationError)?;
    let sum = a + b;
    Ok(bincode::serialize(&sum)? )
}).await;
```

Registering a method again overwrites the previous handler.

### Registering Streaming Handlers

Streaming handlers consume a stream of request payloads and produce a stream of
`Result<Vec<u8>, RpcError>` responses. Use `async_stream::stream!` or
`tokio_stream` helpers to build the return value.

```rust
use async_stream::stream;
use futures::StreamExt;

server.register_streaming("echo_stream", |mut reqs| async move {
    stream! {
        while let Some(payload) = reqs.next().await {
            yield Ok(payload); // echo back exactly what we received
        }
    }
}).await;
```

### Binding and Starting

Binding consumes the TLS material supplied in `RpcConfig` and returns an
`s2n_quic::Server` that feeds into `start`:

```rust
let quic_server = server.bind()?;
println!("listening on {}", server.socket_addr.unwrap());
server.start(quic_server).await?;
```

`start` runs until the QUIC provider stops delivering connections (typically
when your process shuts down). Every accepted connection and stream is served
concurrently.

### Graceful Shutdown

Wrap the `start` future inside a `tokio::select!` with your shutdown signal.
When `accept()` yields `None` the loop exits and the server terminates cleanly.

## Client Essentials

### Connecting

```rust
use rpcnet::{RpcClient, RpcConfig};
use std::net::SocketAddr;

let config = RpcConfig::new("certs/ca.pem", "127.0.0.1:0")
    .with_server_name("localhost");
let server_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
let client = RpcClient::connect(server_addr, config).await?;
```

Client configuration mirrors the server TLS settings, including optional
keep-alive.

### Unary Calls

```rust
let payload = bincode::serialize(&(21, 21))?;
let response = client.call("add", payload).await?;
let result: i32 = bincode::deserialize(&response)?;
assert_eq!(result, 42);
```

Errors surface as `RpcError` values. Timeouts honour the `DEFAULT_TIMEOUT`
constant (30 seconds normally, 2 seconds under `cfg(test)`).

### Concurrent Calls

Clone the client (internally `Arc`) and issue calls in parallel. Each call opens
a new bidirectional stream on the shared connection.

```rust
use std::sync::Arc;
use tokio::join;

let client = Arc::new(client);
let (a, b) = join!(
    client.clone().call("first", vec![]),
    client.clone().call("second", vec![])
);
```

### Inspecting Request IDs

`RpcClient` maintains an atomic `next_id`. Incrementing it per call keeps
request/response pairs aligned. You rarely need to touch this directly, but it
aids traffic debugging.

## Streaming Patterns

RpcNet exposes three streaming helpers built on top of QUIC bidirectional
streams. Each frame is length-prefixed followed by the payload bytes.

### Bidirectional (`call_streaming`)

```rust
use futures::stream;
use futures::StreamExt;

let requests = stream::iter(vec![
    b"hello".to_vec(),
    b"world".to_vec(),
]);

let responses = client.call_streaming("chat", requests).await?;
let mut responses = Box::pin(responses);
while let Some(frame) = responses.next().await {
    println!("response: {:?}", frame?);
}
```

The client sends the method name first, then each payload, finishing with a `0`
length frame to signal completion. Sending continues even as responses arrive;
upload and download directions are independent.

### Server Streaming (`call_server_streaming`)

Server streaming wraps `call_streaming` and sends a single request frame before
yielding the response stream:

```rust
use futures::StreamExt;

let stream = client.call_server_streaming("list_items", Vec::new()).await?;
let mut stream = Box::pin(stream);
while let Some(frame) = stream.next().await {
    println!("item: {:?}", frame?);
}
```

### Client Streaming (`call_client_streaming`)

Client streaming uploads many payloads and waits for an aggregated result.

```rust
use futures::stream;

let uploads = stream::iter(vec![b"chunk-a".to_vec(), b"chunk-b".to_vec()]);
let digest = client.call_client_streaming("upload", uploads).await?;
println!("digest bytes: {digest:?}");
```

### Implementing Streaming Handlers

On the server, build a response stream with `async_stream::stream!` or
`tokio_stream` helpers. Returning `Err` from the response stream maps to a
generic error frame; encode richer error payloads yourself when necessary.

```rust
use async_stream::stream;
use futures::StreamExt;

server.register_streaming("uppercase", |mut reqs| async move {
    stream! {
        while let Some(bytes) = reqs.next().await {
            let mut owned = bytes.clone();
            owned.make_ascii_uppercase();
            yield Ok(owned);
        }
    }
}).await;
```

## Cluster Management (v0.2.0+)

RpcNet provides built-in distributed systems support for building scalable clusters with automatic discovery and failover.

### Architecture Components

#### NodeRegistry

Tracks all nodes in the cluster with their metadata (address, tags, status). Filters nodes by tags for heterogeneous worker pools (e.g., GPU workers, CPU workers).

```rust
use rpcnet::cluster::NodeRegistry;

let registry = NodeRegistry::new(cluster);
let gpu_workers = registry.nodes_with_tag("gpu").await;
```

#### WorkerRegistry

Automatically discovers workers via gossip and provides load-balanced worker selection.

```rust
use rpcnet::cluster::{WorkerRegistry, LoadBalancingStrategy};

let registry = WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
);
registry.start().await;

let worker = registry.select_worker(Some("role=worker")).await?;
```

#### Load Balancing Strategies

- **Round Robin**: Even distribution across workers
- **Random**: Random selection for stateless workloads  
- **Least Connections**: Routes to least-loaded worker (recommended)

#### Health Checking

Phi Accrual failure detector provides accurate, adaptive health monitoring:

```rust
use rpcnet::cluster::HealthChecker;

let health = HealthChecker::new(cluster, config);
health.start().await;

// Automatically marks nodes as failed/recovered
```

#### Connection Pooling

Efficient connection reuse with configurable pool sizes:

```rust
use rpcnet::cluster::{ConnectionPool, PoolConfig};

let pool = ConnectionPool::new(PoolConfig {
    max_connections: 100,
    idle_timeout: Duration::from_secs(60),
    ..Default::default()
});
```

### Gossip Protocol

RpcNet uses SWIM (Scalable Weakly-consistent Infection-style Process Group Membership Protocol) for:
- Automatic node discovery
- Failure detection propagation
- Cluster state synchronization
- Network partition detection

### ClusterClient

High-level client that combines worker discovery and load balancing:

```rust
use rpcnet::cluster::{ClusterClient, WorkerRegistry, LoadBalancingStrategy};

let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));
registry.start().await;

let client = Arc::new(ClusterClient::new(registry, config));

// Call any worker in the pool
let result = client.call_worker("compute", data, Some("role=worker")).await?;
```

### Complete Example

See the [Cluster Example](cluster-example.md) chapter for a complete walkthrough of building a distributed worker pool with automatic discovery, load balancing, and failover.
