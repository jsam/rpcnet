# Python Inference Worker Example

This example demonstrates how to implement an RPC server in Python using RpcNet's generated bindings.

## Overview

- `inference.rpc.rs` - Service definition (unary RPC)
- `generated/inference/` - Auto-generated Python bindings
- `python_worker.py` - Python server implementation
- `test_client.py` - Python test client

## ⚠️ Current Limitation

**Cluster Integration**: The Python bindings do not yet expose the cluster/SWIM gossip functionality. This means:
- ❌ Python workers cannot auto-register with the director via SWIM
- ❌ Python workers won't appear in the cluster member list
- ✅ Python workers can still handle RPC requests when connected directly
- ✅ All RPC functionality (unary, streaming) works correctly

To add full cluster support, the Rust `_rpcnet` extension module would need to expose the cluster APIs.

## Setup

### 1. Build the Python extension module

From the project root:

```bash
maturin develop --features extension-module
```

### 2. Generate Python bindings

```bash
cargo build --bin rpcnet-gen --features codegen,python --release
./target/release/rpcnet-gen --input examples/python/cluster_2/inference.rpc.rs --output examples/python/cluster_2/generated --python
```

### 3. Ensure TLS certificates exist

```bash
# From project root
mkdir -p certs
cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem -days 365 -nodes -subj "/CN=localhost"
cd ..
```

## Running the Example

### Terminal 1: Start the Python Worker

```bash
cd examples/python/cluster_2

WORKER_LABEL=python-worker \
  WORKER_ADDR=127.0.0.1:62002 \
  CERT_PATH=../../../certs/test_cert.pem \
  KEY_PATH=../../../certs/test_key.pem \
  ../../../.venv/bin/python python_worker.py
```

### Terminal 2: Run the Test Client

```bash
cd examples/python/cluster_2

WORKER_ADDR=127.0.0.1:62002 \
  CERT_PATH=../../../certs/test_cert.pem \
  ../../../.venv/bin/python test_client.py
```

## What It Demonstrates

1. **Service Definition**: Rust service trait with async methods
2. **Code Generation**: Automatic Python client/server generation
3. **Type Safety**: Dataclass-based request/response types
4. **Enum Support**: Union types for response variants
5. **Serialization**: Automatic MessagePack serialization
6. **TLS Security**: QUIC+TLS for encrypted communication

## Implementation Details

### Service Handler (`python_worker.py`)

```python
class PythonInferenceWorker(InferenceHandler):
    """Implement the InferenceHandler interface"""

    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        # Business logic here
        return InferenceResponseToken(
            text=f"Processed: {request.prompt}",
            sequence=self.request_count
        )
```

### Generated Types (`generated/inference/types.py`)

- `InferenceRequest` - Request dataclass
- `InferenceResponse` - Union type with variants:
  - `InferenceResponseConnected`
  - `InferenceResponseToken`
  - `InferenceResponseError`
  - `InferenceResponseDone`
- Serialization helpers for enum variants

### Generated Server (`generated/inference/server.py`)

- `InferenceHandler` - Abstract base class
- `InferenceServer` - RPC server wrapper
- Automatic method registration
- MessagePack serialization/deserialization

### Generated Client (`generated/inference/client.py`)

- `InferenceClient` - Type-safe client
- Async method calls
- Automatic serialization

## Extending the Example

To add more RPC methods:

1. Update `inference.rpc.rs` with new methods
2. Regenerate bindings with `rpcnet-gen --python`
3. Implement new methods in your handler class
4. Use the updated client to call new methods

## Troubleshooting

### ModuleNotFoundError: No module named '_rpcnet'

Run `maturin develop --features extension-module` from the project root.

### Connection refused

Ensure the worker is running and the address/port match in both worker and client.

### Certificate errors

Regenerate certificates or ensure paths are correct in environment variables.
