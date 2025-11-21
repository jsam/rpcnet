# RpcNet Python Cluster Example

A distributed inference cluster demonstrating:
- **Rust Director** with SWIM cluster membership and load balancing
- **Python Workers** with multi-process support (TRUE parallelism, no GIL contention)
- **Type-safe RPC** from `.rpc.rs` definitions to Python handlers  
- **Automatic service discovery** via SWIM gossip protocol
- **Production-ready** cluster with health monitoring and failure detection

## Quick Start

### 1. Generate Certificates

```bash
cd ../../..
./generate_certs.sh
cd examples/python/cluster
```

### 2. Build Rust Components

```bash
cargo build --release
```

### 3. Start the Director (Rust)

The director manages the cluster and routes requests to workers.

```bash
DIRECTOR_ADDR=127.0.0.1:61000 \
  target/release/director
```

### 4. Start Python Worker(s) - Multi-Process with Cluster

**Python Worker (Recommended - Multi-Process with True Parallelism)**

```bash
# Terminal 2: Start first Python worker with 11 processes
WORKER_LABEL=python-worker-1 \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=certs/test_cert.pem \
  KEY_PATH=certs/test_key.pem \
  PROCESSES=11 \
  /Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python python_worker.py

# Terminal 3: Start second Python worker (optional)
WORKER_LABEL=python-worker-2 \
  WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=certs/test_cert.pem \
  KEY_PATH=certs/test_key.pem \
  PROCESSES=11 \
  /Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python python_worker.py
```

**Rust Worker (Alternative)**

```bash
# Terminal 2: Start Rust worker
WORKER_LABEL=rust-worker-1 \
  WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 \
  target/release/worker
```

### 5. Test with Client

The client connects to the director, gets a worker assignment, then connects directly to that worker.

```bash
# Terminal 4: Run cluster client
DIRECTOR_ADDR=127.0.0.1:61000 \
  CERT_PATH=certs/test_cert.pem \
  python client.py
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Director (Rust)                      │
│  - SWIM cluster membership                              │
│  - Load balancing (LeastConnections)                    │
│  - Service discovery                                    │
│  - Health monitoring                                    │
└───────────┬────────────────────────────┬────────────────┘
            │ QUIC/TLS                   │ QUIC/TLS
            ▼                            ▼
┌───────────────────────┐    ┌───────────────────────┐
│   Python Worker 1     │    │   Python Worker 2     │
│  ┌─────────────────┐  │    │  ┌─────────────────┐  │
│  │ Master Process  │  │    │  │ Master Process  │  │
│  │  - Event loop   │  │    │  │  - Event loop   │  │
│  │  - Routing      │  │    │  │  - Routing      │  │
│  └────┬────────────┘  │    │  └────┬────────────┘  │
│       │               │    │       │               │
│  ┌────▼──────────┐    │    │  ┌────▼──────────┐    │
│  │ 11 Worker     │    │    │  │ 11 Worker     │    │
│  │ Processes     │    │    │  │ Processes     │    │
│  │ - True        │    │    │  │ - True        │    │
│  │   parallelism │    │    │  │   parallelism │    │
│  │ - No GIL      │    │    │  │ - No GIL      │    │
│  │ - Typed       │    │    │  │ - Typed       │    │
│  │   handlers    │    │    │  │   handlers    │    │
│  └───────────────┘    │    │  └───────────────┘    │
└───────────────────────┘    └───────────────────────┘
```

## Performance Tuning

### Worker Processes

Match worker processes to available CPU cores:

```bash
# Auto-detect CPU count
PROCESSES=$(python -c "import os; print(os.cpu_count())") python python_worker.py

# Or set manually
PROCESSES=11 python python_worker.py
```

### Load Balancing

The director uses `LeastConnections` strategy by default. Workers automatically report their connection count via SWIM cluster tags.

## Configuration

### Environment Variables

**Director:**
- `DIRECTOR_ADDR` - Director bind address (default: `127.0.0.1:61000`)

**Worker (Rust & Python):**
- `WORKER_LABEL` - Worker identifier (default: `python-worker`)
- `WORKER_ADDR` - Worker bind address (default: `127.0.0.1:62002`)
- `DIRECTOR_ADDR` - Director address to connect to (default: `127.0.0.1:61000`)

**Python Worker Only:**
- `CERT_PATH` - TLS certificate path (default: `certs/test_cert.pem`)
- `KEY_PATH` - TLS key path (default: `certs/test_key.pem`)
- `PROCESSES` - Number of worker processes (default: CPU count) - TRUE multi-process with cluster support!

## Service Definitions

### DirectorRegistry Service

Defined in `director_registry.rpc.rs`:

```rust
#[rpcnet::service]
pub trait DirectorRegistry {
    async fn get_worker(
        &self, 
        request: GetWorkerRequest
    ) -> Result<GetWorkerResponse, DirectorError>;
}
```

### Inference Service

Defined in `inference.rpc.rs`:

```rust
#[rpcnet::service]
pub trait Inference {
    async fn infer(
        &self, 
        request: InferenceRequest
    ) -> Result<InferenceResponse, InferenceError>;
}
```

## Python Handler Implementation

Implement typed handlers that are automatically serialized to worker processes:

```python
from inference.server import InferenceHandler, InferenceServer
from inference.types import InferenceRequest, InferenceResponse

class PythonInferenceWorker(InferenceHandler):
    """Handler is serialized with cloudpickle to workers"""
    
    def __init__(self, worker_label: str):
        self.worker_label = worker_label
    
    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        """Runs in worker process with true parallelism"""
        return InferenceResponseToken(
            text=f"Processed: {request.prompt}",
            sequence=0
        )

# Create server with handler
handler = PythonInferenceWorker("my-worker")
config = rpcnet.RpcConfig(...)
server = InferenceServer(handler, config)
await server.serve()
```

## Cluster Features

### SWIM Gossip Protocol

- Automatic failure detection with phi-accrual
- Membership propagation via gossip
- Configurable timeouts and intervals

### Service Discovery

Workers register themselves with tags:
```python
await cluster.update_tags([
    ("role", "worker"),
    ("label", worker_label),
    ("language", "python"),
])
```

### Load Balancing

Director selects workers based on:
- Current connection count
- Worker health status
- Available capacity

## Testing

### Cluster Client

The client demonstrates the proper cluster architecture:
1. Connect to director
2. Request worker assignment
3. Connect directly to assigned worker
4. Make inference requests

```bash
python client.py
```

Output shows the full flow:
```
🔍 Request #1: Asking director for worker assignment...
✅ Connected to director
🔀 Director assigned worker:
   Worker: python-worker-1
   Address: 127.0.0.1:62001
🔌 Establishing direct connection to worker...
✅ Direct connection established to worker
📤 Sending inference request to worker...
📥 Response from worker:
   Python worker 'python-worker-1' (PID 12345) processed: Hello!
```

### Shell Scripts

```bash
# Start entire cluster with logging
./run_cluster_with_logging.sh

# Stop cluster
./stop_cluster.sh

# Test cluster functionality
./test_cluster.sh
```

## Troubleshooting

### Workers Not Discovered

Check director logs for SWIM gossip activity:
```
✅ Cluster enabled - Director is now discoverable
📊 Worker pool status: 2 workers available
   - worker-1 at 127.0.0.1:62001 (0 connections)
   - worker-2 at 127.0.0.1:62002 (0 connections)
```

### Handler Not Defined Error

Ensure `cloudpickle` is installed and Python extension is rebuilt:
```bash
pip install cloudpickle
maturin develop --release
```

Check worker logs for:
```
Successfully unpickled handler instance: PythonInferenceWorker
```

### Connection Timeouts

Verify certificates are valid and paths are correct:
```bash
ls -la certs/test_cert.pem certs/test_key.pem
```

## Comparison with client_server Example

| Feature | client_server | cluster |
|---------|---------------|---------|
| **Workers** | Python multi-process | Python multi-process (or Rust) |
| **Director** | None (direct connection) | Rust with SWIM |
| **Discovery** | Static addresses | SWIM gossip |
| **Load Balancing** | None | LeastConnections |
| **Health Checks** | None | Phi-accrual failure detection |
| **Scalability** | Single server | Multiple workers across machines |
| **Multi-process** | ✅ Yes | ✅ Yes (with cluster!) |

## Next Steps

1. **Scale Horizontally**: Add more Python workers on different machines
2. **Custom Load Balancing**: Implement custom strategies in director
3. **Monitoring**: Add metrics collection for cluster health
4. **Production Deployment**: Use proper certificate management
