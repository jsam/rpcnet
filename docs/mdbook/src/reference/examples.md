# Example Programs

This page indexes all example programs included in the RpcNet repository. Each example demonstrates specific features and can be run locally.

## Repository Structure

All examples are located in the `examples/` directory:

```
examples/
├── cluster/          - Distributed cluster with auto-discovery
├── python/
│   └── cluster/      - Python bindings for cluster example
└── (more to come)
```

## Cluster Example

**Location**: `examples/cluster/`  
**Documentation**: [Cluster Example Chapter](../cluster-example.md)

Demonstrates RpcNet's distributed cluster features with automatic service discovery, load balancing, and failure handling.

### Components

**Director** (`examples/cluster/src/bin/director.rs`)
- Coordinator node for the cluster
- Uses `WorkerRegistry` for auto-discovery
- Implements load-balanced request routing
- Monitors worker pool health

**Worker** (`examples/cluster/src/bin/worker.rs`)
- Processing node that joins cluster automatically
- Tags itself with `role=worker` for discovery
- Handles compute tasks
- Supports failure simulation for testing

**Client** (`examples/cluster/src/bin/client.rs`)
- Connects through director
- Establishes direct connections to workers
- Handles worker failover automatically
- Demonstrates streaming requests

### Quick Start

```bash
# Terminal 1: Start Director
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2: Start Worker A
WORKER_LABEL=worker-a \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 3: Start Worker B
WORKER_LABEL=worker-b \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 4: Run Client
DIRECTOR_ADDR=127.0.0.1:61000 \
  RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client
```

### Features Demonstrated

- ✅ **Automatic Discovery**: Workers join via SWIM gossip protocol
- ✅ **Load Balancing**: Uses `LeastConnections` strategy
- ✅ **Health Checking**: Phi Accrual failure detection
- ✅ **Failover**: Client handles worker failures gracefully
- ✅ **Streaming**: Server-side streaming responses
- ✅ **Tag-Based Routing**: Filter workers by role
- ✅ **Cluster Events**: Monitor node joined/left/failed

### Testing Scenarios

**1. Normal Operation**:
- Start director + 2 workers + client
- Observe load distribution across workers
- Watch streaming responses flow

**2. Worker Failure**:
```bash
# Enable failure simulation
WORKER_FAILURE_ENABLED=true cargo run --bin worker
```
- Worker cycles through failures every ~18 seconds
- Client detects failures and switches workers
- Streaming continues with minimal interruption

**3. Hard Kill**:
- Press `Ctrl+C` on a worker
- Director detects failure via gossip
- Client fails over to remaining workers

**4. Worker Restart**:
- Restart killed worker
- Automatic re-discovery and re-integration
- Load distribution resumes

### Configuration Options

**Director**:
- `DIRECTOR_ADDR` - Bind address (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`)

**Worker**:
- `WORKER_LABEL` - Worker identifier (default: `worker-1`)
- `WORKER_ADDR` - Bind address (default: `127.0.0.1:62001`)
- `DIRECTOR_ADDR` - Director address (default: `127.0.0.1:61000`)
- `WORKER_FAILURE_ENABLED` - Enable failure simulation (default: `false`)
- `RUST_LOG` - Log level

**Client**:
- `DIRECTOR_ADDR` - Director address (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level

### Code Highlights

**Worker Auto-Discovery** (`worker.rs`):
```rust
// Join cluster
let cluster = server.enable_cluster(cluster_config).await?;
cluster.join(vec![director_addr.parse()?]).await?;

// Tag for discovery
cluster.set_tag("role", "worker");
cluster.set_tag("label", &worker_label);
```

**Load-Balanced Selection** (`director.rs`):
```rust
// Create registry with load balancing
let registry = Arc::new(WorkerRegistry::new(
    cluster,
    LoadBalancingStrategy::LeastConnections
));

// Select worker automatically
let worker = registry.select_worker(Some("role=worker")).await?;
```

**Client Failover** (`client.rs`):
```rust
// Try worker
match worker_client.generate(request).await {
    Ok(stream) => {
        // Process stream
    }
    Err(e) => {
        // Worker failed - return to director for new assignment
        println!("Worker failed: {}", e);
        continue;
    }
}
```

## Python Cluster Example

**Location**: `examples/python/cluster/`
**Documentation**: [Python Bindings](../python-bindings.md)

Demonstrates Python code generation from Rust service definitions, enabling Python clients to interact with Rust cluster services.

### Components

**Python Clients**:
- `python_client.py` - Simple example connecting to director
- `python_streaming_client.py` - Full workflow (director → worker → inference)

**Generated Bindings**:
- `generated/directorregistry/` - Python bindings for director service
- `generated/inference/` - Python bindings for worker service

**Service Definitions**:
- `director_registry.rpc.rs` - Director registry service
- `inference.rpc.rs` - Worker inference service

### Prerequisites

```bash
# 1. Generate TLS certificates (if needed)
mkdir -p certs && cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
cd ..

# 2. Build code generator with Python support
cargo build --bin rpcnet-gen --features codegen,python --release

# 3. Generate Python bindings
./target/release/rpcnet-gen \
  --input examples/python/cluster/director_registry.rpc.rs \
  --output examples/python/cluster/generated \
  --python

./target/release/rpcnet-gen \
  --input examples/python/cluster/inference.rpc.rs \
  --output examples/python/cluster/generated \
  --python

# 4. Build Python module
maturin develop --features python --release
```

### Quick Start

```bash
# Terminal 1: Start Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2: Start Worker
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 3: Run Python Client
python examples/python/cluster/python_client.py

# Or run full workflow demo
python examples/python/cluster/python_streaming_client.py
```

### Features Demonstrated

- ✅ **Type-Safe Python API**: Generated dataclasses with type hints
- ✅ **Async/Await**: Native Python asyncio integration
- ✅ **Cross-Language RPC**: Python ↔ Rust communication
- ✅ **MessagePack Serialization**: Binary serialization for efficiency
- ✅ **QUIC+TLS Transport**: Same protocol as Rust services
- ✅ **Error Handling**: Service errors mapped to Python exceptions
- ✅ **Load Balancing**: Multiple workers with round-robin selection

### Example Output

**Simple Client** (`python_client.py`):
```
====================================================================
Python Client for RpcNet Cluster - Director Connection Demo
====================================================================

📁 Using certificate: ../../../certs/test_cert.pem
🎯 Director address: 127.0.0.1:61000

1️⃣  Connecting to director registry...
   ✅ Connected to director at 127.0.0.1:61000

2️⃣  Requesting workers (testing load balancing)...
   Request 1:
      ✅ Worker: worker-a
      📍 Address: 127.0.0.1:62001
      🔗 Connection ID: conn-1234

✅ Python client completed successfully!
```

**Streaming Client** (`python_streaming_client.py`):
```
┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: Connecting to Director Registry                        │
└─────────────────────────────────────────────────────────────────┘
✅ Connected to director at 127.0.0.1:61000

┌─────────────────────────────────────────────────────────────────┐
│ STEP 2: Getting Available Worker                               │
└─────────────────────────────────────────────────────────────────┘
✅ Got worker: worker-a at 127.0.0.1:62001

┌─────────────────────────────────────────────────────────────────┐
│ STEP 3: Connecting to Worker                                   │
└─────────────────────────────────────────────────────────────────┘
✅ Connected to worker at 127.0.0.1:62001

┌─────────────────────────────────────────────────────────────────┐
│ STEP 4: Sending Inference Requests                             │
└─────────────────────────────────────────────────────────────────┘
Request 1/5:
  ✅ Success (45.2ms)
  📝 Prompt:   Hello, how are you?
  📊 Response: I'm doing well, thank you for asking!
  🔧 Worker:   worker-a

✅ Python Streaming Client Demo Completed Successfully!
```

### Code Example

```python
import asyncio
from directorregistry import DirectorRegistryClient, GetWorkerRequest
from inference import InferenceClient, InferenceRequest

async def main():
    # 1. Connect to director
    director = await DirectorRegistryClient.connect(
        "127.0.0.1:61000",
        cert_path="../../../certs/test_cert.pem",
        server_name="localhost"
    )

    # 2. Get available worker
    worker_info = await director.get_worker(
        GetWorkerRequest(connection_id=None, prompt="Test request")
    )

    # 3. Connect to worker
    worker = await InferenceClient.connect(
        worker_info.worker_addr,
        cert_path="../../../certs/test_cert.pem",
        server_name="localhost"
    )

    # 4. Send inference request
    response = await worker.infer(
        InferenceRequest(
            connection_id=worker_info.connection_id,
            prompt="Hello from Python!"
        )
    )
    print(f"Response: {response.response}")

asyncio.run(main())
```

### Documentation

- **`examples/python/cluster/README.md`** - Complete usage guide
- **`examples/python/cluster/QUICKSTART.md`** - Quick start guide
- **`examples/python/cluster/SUMMARY.md`** - Feature summary

### Troubleshooting

**"Module not found: _rpcnet"**
```bash
maturin develop --features python --release
```

**"Unknown method: Registry.get_worker"**
- Ensure you're using actual service definitions from `examples/cluster/`
- Regenerate Python bindings after copying service files

**"Connection refused"**
- Start Rust cluster first (director + worker)
- Check that ports 61000 (director) and 62001 (worker) are available

## Running Examples from Repository

### Prerequisites

1. **Clone repository**:
```bash
git clone https://github.com/yourusername/rpcnet.git
cd rpcnet
```

2. **Generate test certificates**:
```bash
mkdir certs
cd certs
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout test_key.pem -out test_cert.pem \
  -days 365 -subj "/CN=localhost"
cd ..
```

3. **Install dependencies**:
```bash
cargo build --examples
```

### Run Specific Example

```bash
# Cluster example
cd examples/cluster
cargo run --bin director
cargo run --bin worker
cargo run --bin client
```

## Creating Your Own Examples

### Basic Template

```rust
// examples/my_example/Cargo.toml
[package]
name = "my_example"
version = "0.1.0"
edition = "2021"

[dependencies]
rpcnet = { path = "../..", features = ["cluster"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"

[[bin]]
name = "server"
path = "src/bin/server.rs"

[[bin]]
name = "client"
path = "src/bin/client.rs"
```

### Example Structure

```
examples/my_example/
├── Cargo.toml
├── README.md
├── my_service.rpc.rs          # RPC trait definition
├── src/
│   ├── lib.rs
│   ├── generated/             # Generated code
│   │   └── my_service.rs
│   └── bin/
│       ├── server.rs
│       └── client.rs
└── tests/
    └── integration_tests.rs
```

### Generate Code

```bash
cd examples/my_example
rpcnet-gen --input my_service.rpc.rs --output src/generated
```

### Document Your Example

Create `examples/my_example/README.md`:

```markdown
# My Example

Brief description of what this example demonstrates.

## Features

- Feature 1
- Feature 2

## Running

Terminal 1:
\`\`\`bash
cargo run --bin server
\`\`\`

Terminal 2:
\`\`\`bash
cargo run --bin client
\`\`\`

## Expected Output

...
```

## Testing Examples

### Manual Testing

```bash
# Run example
cd examples/cluster
cargo run --bin director &
cargo run --bin worker &
cargo run --bin client

# Verify output
# Clean up
killall director worker
```

### Integration Tests

```bash
# Run example's tests
cd examples/cluster
cargo test

# Run all example tests
cargo test --examples
```

## Example Comparison

| Example | Complexity | Features | Best For |
|---------|-----------|----------|----------|
| **cluster** | Intermediate | Discovery, Load Balancing, Failover, Streaming | Understanding distributed systems |
| **python/cluster** | Beginner | Python Bindings, Type Safety, Async API | Cross-language RPC, Python integration |

## Common Issues

### Certificate Errors

```
Error: Certificate verification failed
```

**Solution**: Ensure certificates exist in `certs/`:
```bash
ls certs/test_cert.pem certs/test_key.pem
```

### Port Already in Use

```
Error: Address already in use (os error 48)
```

**Solution**: Kill existing processes or change port:
```bash
lsof -ti:61000 | xargs kill
# or
DIRECTOR_ADDR=127.0.0.1:61001 cargo run --bin director
```

### Workers Not Discovered

```
Error: No workers available
```

**Solution**: 
1. Start director first (seed node)
2. Wait 2-3 seconds for gossip propagation
3. Check firewall allows UDP port 7946

## Contributing Examples

Want to contribute an example? Great! Here's how:

1. **Create example directory**: `examples/your_example/`
2. **Write code**: Follow structure above
3. **Test thoroughly**: Include integration tests
4. **Document well**: Clear README with running instructions
5. **Submit PR**: Include example in this index

**Good example ideas**:
- Basic client-server RPC
- Bidirectional streaming
- Multi-region deployment
- Custom load balancing strategy
- Monitoring and metrics integration

## Next Steps

- **[Cluster Tutorial](../cluster/tutorial.md)** - Build cluster from scratch
- **[API Reference](api.md)** - API documentation
- **[GitHub Repository](https://github.com/yourusername/rpcnet)** - Browse all examples

## Video Walkthroughs

Coming soon! Video walkthroughs demonstrating:
- Running the cluster example
- Testing failure scenarios
- Building your own example
