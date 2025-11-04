# Python Cluster Example - Generated Bindings

This directory demonstrates **Python code generation** from RpcNet service definitions using the `--python` flag.

## Overview

This example shows how to:
1. Define RPC services in `.rpc.rs` files
2. Generate Python client/server code with `rpcnet-gen --python`
3. Use the generated Python code to interact with Rust services

**Note**: The actual cluster (director, workers) runs in **Rust** (see `examples/cluster/`). The Python code here shows how Python clients could interact with the cluster.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Python Client (using generated bindings)      │
│  - Connects to Rust director                   │
│  - Makes RPC calls using Python async/await    │
└──────────────────┬──────────────────────────────┘
                   │ RPC over QUIC+TLS
┌──────────────────▼──────────────────────────────┐
│  Rust Director (examples/cluster/director)     │
│  - Registry service (load balancing)           │
│  - Cluster management                          │
└──────────────────┬──────────────────────────────┘
                   │
         ┌─────────┴─────────┐
         │                   │
┌────────▼────────┐  ┌───────▼────────┐
│  Rust Worker A  │  │  Rust Worker B │
│  - Compute svc  │  │  - Compute svc │
└─────────────────┘  └────────────────┘
```

## Generated Code Structure

```
generated/
├── compute/          # Compute service (worker API)
│   ├── __init__.py
│   ├── types.py      # ComputeRequest, ComputeResponse, ComputeError
│   ├── client.py     # ComputeClient for calling workers
│   └── server.py     # ComputeServer for implementing workers
│
└── registry/         # Registry service (director API)
    ├── __init__.py
    ├── types.py      # GetWorkerRequest, GetWorkerResponse, RegistryError
    ├── client.py     # RegistryClient for calling director
    └── server.py     # RegistryServer for implementing director
```

## Service Definitions

### `compute.rpc.rs` - Worker Compute Service

```rust
#[rpcnet::service]
pub trait Compute {
    async fn process(
        &self,
        request: ComputeRequest
    ) -> Result<ComputeResponse, ComputeError>;
}
```

**Python Usage:**
```python
from generated.compute import ComputeClient, ComputeRequest

# Connect to worker
client = await ComputeClient.connect(
    "127.0.0.1:62001",
    cert_path="certs/test_cert.pem",
    server_name="localhost"
)

# Call compute service
request = ComputeRequest(task_id="task-1", data="process this")
response = await client.process(request)
print(f"Result: {response.result} from {response.worker_id}")
```

### `registry.rpc.rs` - Director Registry Service

```rust
#[rpcnet::service]
pub trait Registry {
    async fn get_worker(
        &self,
        request: GetWorkerRequest
    ) -> Result<GetWorkerResponse, RegistryError>;
}
```

**Python Usage:**
```python
from generated.registry import RegistryClient, GetWorkerRequest

# Connect to director
client = await RegistryClient.connect(
    "127.0.0.1:61000",
    cert_path="certs/test_cert.pem",
    server_name="localhost"
)

# Get an available worker
request = GetWorkerRequest(client_id="python-client")
response = await client.get_worker(request)
print(f"Got worker: {response.worker_addr}")
```

## Generating Python Code

```bash
# From project root directory

# Generate Compute service bindings
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/python/cluster/compute.rpc.rs \
  --output examples/python/cluster/generated \
  --python

# Generate Registry service bindings
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/python/cluster/registry.rpc.rs \
  --output examples/python/cluster/generated \
  --python
```

## Running the Example

### 1. Start the Rust Cluster

The actual cluster runs in Rust. See `examples/cluster/README.md` for details:

```bash
# Terminal 1 - Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Worker A
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 3 - Worker B
WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

### 2. Use Python Client (Optional)

Once the Rust cluster is running, you can interact with it from Python:

```bash
# Install Python dependencies
cd examples/python/cluster
pip install -r requirements.txt

# Build Python bindings
cd ../../..  # Back to project root
maturin develop --features python --release

# Run Python client
python examples/python/cluster/python_client.py
```

## Python Client Example

See `python_client.py` for a complete example:

```python
import asyncio
from generated.registry import RegistryClient, GetWorkerRequest
from generated.compute import ComputeClient, ComputeRequest

async def main():
    # 1. Connect to director
    director = await RegistryClient.connect(
        "127.0.0.1:61000",
        cert_path="certs/test_cert.pem",
        server_name="localhost"
    )

    # 2. Get available worker
    worker_info = await director.get_worker(
        GetWorkerRequest(client_id="python-client")
    )
    print(f"Got worker: {worker_info.worker_addr}")

    # 3. Connect to worker
    worker = await ComputeClient.connect(
        worker_info.worker_addr,
        cert_path="certs/test_cert.pem",
        server_name="localhost"
    )

    # 4. Send compute task
    response = await worker.process(
        ComputeRequest(
            task_id="task-1",
            data="Hello from Python!"
        )
    )
    print(f"Result: {response.result}")

asyncio.run(main())
```

## Features Demonstrated

### 1. Type-Safe Python API

Generated code provides full type safety:
- Request/Response dataclasses
- Async client methods
- Error handling with typed exceptions

### 2. Async/Await Support

All RPC calls are async and integrate with Python's `asyncio`:
```python
response = await client.process(request)  # Non-blocking!
```

### 3. Automatic Serialization

Request/response objects are automatically serialized:
```python
# Python objects...
request = ComputeRequest(task_id="1", data="test")

# ...automatically converted to bytes for RPC
response = await client.process(request)

# ...and back to Python objects
print(response.result)  # Deserialized automatically!
```

### 4. Error Handling

Service errors are mapped to Python exceptions:
```python
try:
    response = await client.process(request)
except ComputeError.WorkerBusy:
    print("Worker is busy, retry later")
except ComputeError.ProcessingFailed as e:
    print(f"Processing failed: {e}")
```

## Comparison with Rust Implementation

| Feature | Rust (`examples/cluster/`) | Python (this example) |
|---------|---------------------------|----------------------|
| **Performance** | ⚡ Native speed | 🐍 Python overhead |
| **Async** | Tokio | asyncio |
| **Types** | Compile-time checked | Runtime checked |
| **Serialization** | bincode (Rust↔Rust) | MessagePack (Python↔Rust) |
| **Use Case** | Production services | Scripting, tools, clients |

## Code Generation Options

```bash
# Generate only client code
rpcnet-gen --input compute.rpc.rs --output generated --python --client-only

# Generate only server code
rpcnet-gen --input compute.rpc.rs --output generated --python --server-only

# Generate only types
rpcnet-gen --input compute.rpc.rs --output generated --python --types-only
```

## Next Steps

1. **Implement Python Worker**: Create a Python worker that implements `ComputeServer`
2. **Load Balancing**: Python client that tests load balancing across workers
3. **Monitoring**: Python script to monitor cluster health
4. **Streaming**: Add streaming RPC examples (server/client/bidirectional)

## Files

- `compute.rpc.rs` - Compute service definition
- `registry.rpc.rs` - Registry service definition
- `generated/` - Generated Python bindings
- `python_client.py` - Example Python client
- `requirements.txt` - Python dependencies
- `README.md` - This file

## See Also

- Main cluster example: `examples/cluster/`
- Python bindings docs: `PYTHON_BINDINGS_COMPLETE.md`
- Code generation docs: `docs/codegen.md`

## Summary

This example shows how to:
- ✅ Define RPC services in Rust
- ✅ Generate type-safe Python bindings
- ✅ Call Rust services from Python
- ✅ Use async/await in Python
- ✅ Handle errors gracefully

The generated Python code provides a Pythonic API for interacting with RpcNet services!
