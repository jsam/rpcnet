# Cluster Tutorial

This hands-on tutorial guides you through building a complete distributed RPC cluster from scratch. You'll create a coordinator (director) that manages a pool of worker nodes, with automatic discovery, load balancing, and failure handling.

## What You'll Build

By the end of this tutorial, you'll have:

- **Director**: Coordinator node that manages worker discovery and routes client requests
- **Workers**: Processing nodes that join automatically and handle compute tasks
- **Client**: Application that connects through the director and handles failover
- **Failure Testing**: Simulate worker failures and observe automatic recovery

**Time**: ~30 minutes  
**Difficulty**: Intermediate

## Prerequisites

### 1. Install RpcNet

```bash
cargo install rpcnet
```

This installs both the library and the `rpcnet-gen` CLI tool.

### 2. Create Test Certificates

RpcNet requires TLS certificates. For development:

```bash
mkdir certs
cd certs

# Generate self-signed certificate
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout test_key.pem -out test_cert.pem \
  -days 365 -subj "/CN=localhost"

cd ..
```

### 3. Create Project Structure

```bash
cargo new --bin cluster_tutorial
cd cluster_tutorial

# Add RpcNet dependency
cargo add rpcnet --features cluster
cargo add tokio --features full
cargo add anyhow
```

Your `Cargo.toml` should include:

```toml
[dependencies]
rpcnet = { version = "0.2", features = ["cluster"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

## Step 1: Define the RPC Interface

Create `compute.rpc.rs` to define the worker interface:

```rust
use rpcnet::prelude::*;

#[rpc_trait]
pub trait ComputeService {
    async fn process_task(&self, task_id: String, data: Vec<u8>) -> Result<ComputeResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResult {
    pub task_id: String,
    pub result: Vec<u8>,
    pub worker_label: String,
}
```

**Generate code**:

```bash
rpcnet-gen --input compute.rpc.rs --output src/generated
```

This creates `src/generated/compute_service.rs` with client and server stubs.

## Step 2: Implement the Worker

Create `src/bin/worker.rs`:

```rust
use anyhow::Result;
use rpcnet::prelude::*;
use rpcnet::cluster::{ClusterMembership, ClusterConfig};
use std::sync::Arc;
use std::env;

mod generated;
use generated::compute_service::*;

struct WorkerHandler {
    label: String,
}

#[rpc_impl]
impl ComputeService for WorkerHandler {
    async fn process_task(&self, task_id: String, data: Vec<u8>) -> Result<ComputeResult> {
        println!("📋 [{}] Processing task: {}", self.label, task_id);
        
        // Simulate work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Return result with worker identity
        Ok(ComputeResult {
            task_id,
            result: data, // Echo data for demo
            worker_label: self.label.clone(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    // Get configuration from environment
    let worker_label = env::var("WORKER_LABEL").unwrap_or_else(|_| "worker-1".to_string());
    let worker_addr = env::var("WORKER_ADDR").unwrap_or_else(|_| "127.0.0.1:62001".to_string());
    let director_addr = env::var("DIRECTOR_ADDR").unwrap_or_else(|_| "127.0.0.1:61000".to_string());
    
    println!("👷 Starting Worker '{}' at {}", worker_label, worker_addr);
    
    // Load certificates
    let cert = std::fs::read("certs/test_cert.pem")?;
    let key = std::fs::read("certs/test_key.pem")?;
    
    // Create RPC server
    let config = ServerConfig::builder()
        .with_cert_and_key(cert, key)?
        .build();
    
    let mut server = Server::new(config);
    
    // Register compute handler
    let handler = Arc::new(WorkerHandler {
        label: worker_label.clone(),
    });
    server.register_service(handler);
    
    // Bind server
    println!("🔌 Binding server to {}...", worker_addr);
    server.bind(&worker_addr).await?;
    println!("✅ Server bound successfully");
    
    // Enable cluster and join
    println!("🌐 Enabling cluster, connecting to director at {}...", director_addr);
    let cluster_config = ClusterConfig::default()
        .with_bind_addr(worker_addr.parse()?);
    
    let cluster = server.enable_cluster(cluster_config).await?;
    cluster.join(vec![director_addr.parse()?]).await?;
    println!("✅ Cluster enabled, connected to director");
    
    // Tag worker for discovery
    println!("🏷️  Tagging worker with role=worker and label={}...", worker_label);
    cluster.set_tag("role", "worker");
    cluster.set_tag("label", &worker_label);
    println!("✅ Worker '{}' joined cluster with role=worker", worker_label);
    
    println!("🚀 Worker '{}' is running and ready to handle requests", worker_label);
    
    // Run server
    server.run().await?;
    
    Ok(())
}
```

## Step 3: Implement the Director

Create `src/bin/director.rs`:

```rust
use anyhow::Result;
use rpcnet::prelude::*;
use rpcnet::cluster::{
    ClusterMembership, ClusterConfig, WorkerRegistry, 
    LoadBalancingStrategy, ClusterClient, ClusterClientConfig
};
use std::sync::Arc;
use std::env;

mod generated;
use generated::compute_service::*;

#[rpc_trait]
pub trait DirectorService {
    async fn get_worker(&self) -> Result<String>;
}

struct DirectorHandler {
    registry: Arc<WorkerRegistry>,
}

#[rpc_impl]
impl DirectorService for DirectorHandler {
    async fn get_worker(&self) -> Result<String> {
        println!("📨 Client requesting worker assignment");
        
        // Select worker using registry
        let worker = self.registry
            .select_worker(Some("role=worker"))
            .await
            .map_err(|e| anyhow::anyhow!("No workers available: {}", e))?;
        
        println!("✅ Assigned worker: {} at {}", worker.label, worker.addr);
        Ok(worker.addr.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let director_addr = env::var("DIRECTOR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string());
    
    println!("🎯 Starting Director at {}", director_addr);
    
    // Load certificates
    println!("📁 Loading certificates from certs/");
    let cert = std::fs::read("certs/test_cert.pem")?;
    let key = std::fs::read("certs/test_key.pem")?;
    
    // Create server
    let config = ServerConfig::builder()
        .with_cert_and_key(cert, key)?
        .build();
    
    let mut server = Server::new(config);
    
    // Enable cluster first
    let cluster_config = ClusterConfig::default()
        .with_bind_addr(director_addr.parse()?);
    
    let cluster = server.enable_cluster(cluster_config).await?;
    println!("✅ Director registered itself in cluster");
    println!("✅ Cluster enabled - Director is now discoverable");
    
    // Create worker registry with load balancing
    let registry = Arc::new(WorkerRegistry::new(
        cluster,
        LoadBalancingStrategy::LeastConnections
    ));
    registry.start().await;
    
    println!("🔄 Load balancing strategy: LeastConnections");
    
    // Register director service
    let handler = Arc::new(DirectorHandler {
        registry: registry.clone(),
    });
    server.register_service(handler);
    
    // Bind and run
    server.bind(&director_addr).await?;
    
    // Monitor worker pool
    tokio::spawn({
        let registry = registry.clone();
        async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let workers = registry.workers().await;
                println!("📊 Worker pool status: {} workers available", workers.len());
                for worker in workers {
                    println!("   - {} at {} ({} connections)", 
                        worker.label, worker.addr, worker.active_connections);
                }
            }
        }
    });
    
    println!("🚀 Director ready - listening on {}", director_addr);
    
    server.run().await?;
    
    Ok(())
}
```

## Step 4: Implement the Client

Create `src/bin/client.rs`:

```rust
use anyhow::Result;
use rpcnet::prelude::*;
use std::env;

mod generated;
use generated::compute_service::*;
use generated::director_service::*;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let director_addr = env::var("DIRECTOR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string());
    
    println!("📡 Starting Client - connecting to director at {}", director_addr);
    
    // Load certificate for TLS
    let cert = std::fs::read("certs/test_cert.pem")?;
    
    let config = ClientConfig::builder()
        .with_server_cert(cert)?
        .build();
    
    // Connect to director
    let director_client = DirectorClient::connect(&director_addr, config.clone()).await?;
    println!("✅ Connected to director");
    
    // Main loop: get worker, process tasks, handle failures
    let mut task_counter = 0;
    loop {
        // Get worker assignment from director
        println!("🔍 Asking director for worker assignment");
        let worker_addr = match director_client.get_worker().await {
            Ok(addr) => {
                println!("🔀 Director assigned worker at {}", addr);
                addr
            }
            Err(e) => {
                println!("❌ Failed to get worker: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        
        // Connect to worker directly
        println!("✅ Establishing direct connection to worker");
        let worker_client = match ComputeClient::connect(&worker_addr, config.clone()).await {
            Ok(client) => {
                println!("✅ Direct connection established");
                client
            }
            Err(e) => {
                println!("❌ Failed to connect to worker: {}", e);
                continue;
            }
        };
        
        // Process tasks until worker fails
        loop {
            task_counter += 1;
            let task_id = format!("task-{}", task_counter);
            let data = format!("data-{}", task_counter).into_bytes();
            
            println!("📤 Sending task: {}", task_id);
            
            match worker_client.process_task(task_id.clone(), data).await {
                Ok(result) => {
                    println!("✅ Task {} completed by worker: {}", 
                        result.task_id, result.worker_label);
                    
                    // Wait before next task
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    println!("⚠️  Worker failed: {} - returning to director", e);
                    break; // Get new worker from director
                }
            }
        }
    }
}
```

## Step 5: Update Cargo.toml

Add the binary definitions to `Cargo.toml`:

```toml
[[bin]]
name = "director"
path = "src/bin/director.rs"

[[bin]]
name = "worker"
path = "src/bin/worker.rs"

[[bin]]
name = "client"
path = "src/bin/client.rs"
```

Also add the generated module to `src/lib.rs`:

```rust
pub mod generated;
```

## Step 6: Run the Cluster

Open **four terminals** and run each component:

### Terminal 1: Start Director

```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin director
```

Wait for: `🚀 Director ready - listening on 127.0.0.1:61000`

### Terminal 2: Start Worker A

```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin worker
```

Wait for: `🚀 Worker 'worker-a' is running and ready to handle requests`

### Terminal 3: Start Worker B

```bash
WORKER_LABEL=worker-b \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin worker
```

Wait for: `🚀 Worker 'worker-b' is running and ready to handle requests`

### Terminal 4: Run Client

```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin client
```

## Step 7: Observe the System

### Director Output

```
🎯 Starting Director at 127.0.0.1:61000
📁 Loading certificates from certs/
✅ Director registered itself in cluster
✅ Cluster enabled - Director is now discoverable
🔄 Load balancing strategy: LeastConnections
🚀 Director ready - listening on 127.0.0.1:61000
📊 Worker pool status: 2 workers available
   - worker-a at 127.0.0.1:62001 (0 connections)
   - worker-b at 127.0.0.1:62002 (0 connections)
📨 Client requesting worker assignment
✅ Assigned worker: worker-a at 127.0.0.1:62001
```

### Worker Output

```
👷 Starting Worker 'worker-a' at 127.0.0.1:62001
🔌 Binding server to 127.0.0.1:62001...
✅ Server bound successfully
🌐 Enabling cluster, connecting to director at 127.0.0.1:61000...
✅ Cluster enabled, connected to director
🏷️  Tagging worker with role=worker and label=worker-a...
✅ Worker 'worker-a' joined cluster with role=worker
🚀 Worker 'worker-a' is running and ready to handle requests
📋 [worker-a] Processing task: task-1
📋 [worker-a] Processing task: task-2
```

### Client Output

```
📡 Starting Client - connecting to director at 127.0.0.1:61000
✅ Connected to director
🔍 Asking director for worker assignment
🔀 Director assigned worker at 127.0.0.1:62001
✅ Establishing direct connection to worker
✅ Direct connection established
📤 Sending task: task-1
✅ Task task-1 completed by worker: worker-a
📤 Sending task: task-2
✅ Task task-2 completed by worker: worker-a
```

## Step 8: Test Failure Handling

### Scenario 1: Kill a Worker

In Worker A terminal, press **Ctrl+C** to kill it.

**Observe**:
- Director detects failure via gossip: `Node worker-a failed`
- Director updates worker pool: `📊 Worker pool status: 1 workers available`
- Client detects error: `⚠️ Worker failed - returning to director`
- Client gets new worker: `🔀 Director assigned worker at 127.0.0.1:62002`
- Tasks continue on Worker B with no data loss

### Scenario 2: Restart Worker

Restart Worker A:

```bash
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --bin worker
```

**Observe**:
- Worker rejoins automatically
- Gossip spreads availability
- Director adds back to pool: `📊 Worker pool status: 2 workers available`
- Future client requests can use either worker

## What You Learned

Congratulations! You've built a complete distributed RPC cluster. You now understand:

✅ **Automatic Discovery**: Workers join via gossip, no manual registration  
✅ **Load Balancing**: Director uses LeastConnections strategy automatically  
✅ **Failure Detection**: Gossip protocol detects and handles node failures  
✅ **Client Failover**: Clients handle worker failures gracefully  
✅ **Tag-Based Routing**: Filter workers by role (`role=worker`)

## Next Steps

### Add More Workers

Scale up by adding more workers with different labels:

```bash
WORKER_LABEL=worker-c \
  WORKER_ADDR=127.0.0.1:62003 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  cargo run --bin worker
```

### Try Different Load Balancing

Change the strategy in `director.rs`:

```rust
LoadBalancingStrategy::RoundRobin       // Even distribution
LoadBalancingStrategy::Random           // Random selection
LoadBalancingStrategy::LeastConnections // Pick least loaded (default)
```

### Add Custom Tags

Tag workers by capability:

```rust
cluster.set_tag("gpu", "true");
cluster.set_tag("zone", "us-west");
```

Then filter in client:

```rust
registry.select_worker(Some("gpu=true")).await?;
```

### Monitor Cluster Events

Subscribe to events in director or workers:

```rust
let mut events = cluster.subscribe();
while let Some(event) = events.recv().await {
    match event {
        ClusterEvent::NodeJoined(node) => println!("Node joined: {:?}", node),
        ClusterEvent::NodeLeft(node) => println!("Node left: {:?}", node),
        ClusterEvent::NodeFailed(node) => println!("Node failed: {:?}", node),
    }
}
```

## Further Reading

- **[Discovery](discovery.md)** - Learn how SWIM gossip protocol works
- **[Load Balancing](load-balancing.md)** - Deep dive into strategies
- **[Health Checking](health.md)** - Understand Phi Accrual algorithm
- **[Connection Pooling](pooling.md)** - Optimize connection reuse
- **[Failure Handling](failures.md)** - Advanced partition detection

Or explore the **[Complete Cluster Example](../cluster-example.md)** with streaming and advanced features.
