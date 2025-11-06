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

This example includes generated Python bindings for the actual cluster services:

```
generated/
├── directorregistry/  # Director registry service (coordinator)
│   ├── __init__.py
│   ├── types.py       # GetWorkerRequest, GetWorkerResponse, DirectorError
│   ├── client.py      # DirectorRegistryClient
│   └── server.py      # DirectorRegistryServer
│
└── inference/         # Inference service (worker)
    ├── __init__.py
    ├── types.py       # InferenceRequest, InferenceResponse, InferenceError
    ├── client.py      # InferenceClient
    └── server.py      # InferenceServer
```

These bindings are generated from the **actual service definitions** used by the running Rust cluster in `examples/cluster/`.

## Service Definitions

### `director_registry.rpc.rs` - Director Registry Service

This is the **actual service** used by the running Rust director:

```rust
#[rpcnet::service]
pub trait DirectorRegistry {
    async fn get_worker(
        &self,
        request: GetWorkerRequest
    ) -> Result<GetWorkerResponse, DirectorError>;
}
```

**Python Usage:**
```python
from directorregistry import DirectorRegistryClient, GetWorkerRequest

# Connect to director
director = await DirectorRegistryClient.connect(
    "127.0.0.1:61000",
    cert_path="../../../certs/test_cert.pem",
    server_name="localhost"
)

# Get an available worker
worker_info = await director.get_worker(
    GetWorkerRequest(
        connection_id=None,
        prompt="Request from Python"
    )
)

if worker_info.success:
    print(f"Got worker: {worker_info.worker_label} at {worker_info.worker_addr}")
```

### `inference.rpc.rs` - Worker Inference Service

This is the **actual service** used by the running Rust workers:

```rust
#[rpcnet::service]
pub trait Inference {
    async fn infer(
        &self,
        request: InferenceRequest
    ) -> Result<InferenceResponse, InferenceError>;
}
```

**Python Usage:**
```python
from inference import InferenceClient, InferenceRequest

# Connect to worker (get address from director first)
worker = await InferenceClient.connect(
    worker_info.worker_addr,
    cert_path="../../../certs/test_cert.pem",
    server_name="localhost"
)

# Send inference request
response = await worker.infer(
    InferenceRequest(
        connection_id=worker_info.connection_id,
        prompt="Hello from Python!"
    )
)
print(f"Response: {response.response} from {response.worker_label}")
```

## Generating Python Code

**Important**: The Python bindings must match the actual Rust cluster services.

```bash
# From project root directory

# 1. Build the code generator
cargo build --release --bin rpcnet-gen --features codegen,python

# 2. Generate DirectorRegistry service bindings (matches running director)
./target/release/rpcnet-gen \
  --input examples/python/cluster/director_registry.rpc.rs \
  --output examples/python/cluster/generated \
  --python

# 3. Generate Inference service bindings (matches running workers)
./target/release/rpcnet-gen \
  --input examples/python/cluster/inference.rpc.rs \
  --output examples/python/cluster/generated \
  --python
```

**Note**: The service definitions (`director_registry.rpc.rs`, `inference.rpc.rs`) are copied from `examples/cluster/` to ensure they match the running services.

## Running the Example

### Prerequisites

1. **Generate TLS Certificates** (if not already done):
```bash
mkdir -p certs
cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
cd ..
```

2. **Build Python Bindings**:
```bash
# From project root
maturin develop --features python --release
```

3. **Install Python Dependencies**:
```bash
pip install -r examples/python/cluster/requirements.txt
```

### Step 1: Start the Rust Cluster

The Python clients connect to the actual Rust cluster. Start the cluster components in separate terminals:

**Terminal 1 - Director (Coordinator)**:
```bash
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director
```

**Terminal 2 - Worker A**:
```bash
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

**Terminal 3 - Worker B (Optional - for load balancing demo)**:
```bash
WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

### Step 2: Run Python Clients

Once the Rust cluster is running, test the Python clients:

**Simple Client (Director only)**:
```bash
python examples/python/cluster/python_client.py
```

**Streaming Client (Full workflow - Director + Worker)**:
```bash
python examples/python/cluster/python_streaming_client.py
```

## Python Client Examples

### Simple Client (`python_client.py`)

Demonstrates connecting to the director and requesting workers:

```python
import asyncio
from directorregistry import DirectorRegistryClient, GetWorkerRequest

async def main():
    # Connect to director
    director = await DirectorRegistryClient.connect(
        "127.0.0.1:61000",
        cert_path="../../../certs/test_cert.pem",
        server_name="localhost"
    )

    # Request workers (tests load balancing)
    for i in range(5):
        worker_info = await director.get_worker(
            GetWorkerRequest(
                connection_id=None,
                prompt=f"Request {i+1} from Python"
            )
        )

        if worker_info.success:
            print(f"Request {i+1}: {worker_info.worker_label} at {worker_info.worker_addr}")

asyncio.run(main())
```

### Streaming Client (`python_streaming_client.py`)

Demonstrates the full end-to-end workflow:

1. Connect to director registry
2. Get available worker
3. Connect to worker
4. Send inference requests
5. Test load balancing

```python
# 1. Get worker from director
director = await DirectorRegistryClient.connect("127.0.0.1:61000", ...)
worker_info = await director.get_worker(GetWorkerRequest(...))

# 2. Connect to worker
worker = await InferenceClient.connect(worker_info.worker_addr, ...)

# 3. Send inference request
response = await worker.infer(
    InferenceRequest(
        connection_id=worker_info.connection_id,
        prompt="Hello from Python!"
    )
)
print(f"Response: {response.response}")
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

- `director_registry.rpc.rs` - Director registry service (from examples/cluster/)
- `inference.rpc.rs` - Worker inference service (from examples/cluster/)
- `generated/` - Generated Python bindings
  - `directorregistry/` - Director client bindings
  - `inference/` - Worker client bindings
- `python_client.py` - Simple example (director only)
- `python_streaming_client.py` - Full workflow example (director + worker)
- `requirements.txt` - Python dependencies
- `README.md`, `QUICKSTART.md`, `SUMMARY.md` - Documentation

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
