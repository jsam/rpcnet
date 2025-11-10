# Python Code Generation

RpcNet supports generating **type-safe Python bindings** from Rust service definitions. This enables Python clients and servers to communicate with Rust services using the same QUIC+TLS transport, with automatic serialization via MessagePack.

## Overview

The `rpcnet-gen` CLI can generate Python client and server code from `.rpc.rs` service definitions:

```bash
rpcnet-gen --input service.rpc.rs --output generated/ --python
```

This produces a Python package with:
- Type-safe dataclasses for requests/responses/errors
- Async client with typed methods
- Server base class for implementing services in Python
- Automatic MessagePack serialization for cross-language compatibility

## Quick Example

### 1. Define Service in Rust

```rust
// greeting.rpc.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreetRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreetResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GreetError {
    InvalidName(String),
}

#[rpcnet::service]
pub trait Greeting {
    async fn greet(&self, request: GreetRequest)
        -> Result<GreetResponse, GreetError>;
}
```

### 2. Generate Python Bindings

```bash
# Build code generator with Python support
cargo build --bin rpcnet-gen --features codegen,python --release

# Generate Python bindings
./target/release/rpcnet-gen \
  --input greeting.rpc.rs \
  --output generated \
  --python
```

### 3. Build Python Module

The Python bindings require the `_rpcnet` native module (PyO3-based):

```bash
# Install maturin if needed
pip install maturin

# Build and install the native module
maturin develop --features python --release
```

### 4. Use in Python

```python
import asyncio
from greeting import GreetingClient, GreetRequest

async def main():
    # Connect to Rust service
    client = await GreetingClient.connect(
        "127.0.0.1:50051",
        cert_path="certs/test_cert.pem",
        server_name="localhost"
    )

    # Make RPC call
    response = await client.greet(
        GreetRequest(name="Alice")
    )

    print(response.message)  # "Hello, Alice!"

asyncio.run(main())
```

## Generated Code Structure

For a service named `Greeting`, the generator produces:

```
generated/
└── greeting/
    ├── __init__.py      # Package exports
    ├── types.py         # GreetRequest, GreetResponse, GreetError
    ├── client.py        # GreetingClient
    └── server.py        # GreetingServer
```

### Types Module (`types.py`)

Python dataclasses with type hints:

```python
from dataclasses import dataclass
from enum import Enum
from typing import Optional, Union

@dataclass
class GreetRequest:
    name: str

@dataclass
class GreetResponse:
    message: str

# Simple enums (no associated data) → Python Enum
class GreetErrorSimple(Enum):
    InvalidName = "InvalidName"

# Complex enums (with associated data) → Union of dataclasses
@dataclass
class GreetErrorInvalidName:
    reason: str

GreetError = Union[GreetErrorInvalidName, ...]
```

### Client Module (`client.py`)

Async client with typed methods:

```python
class GreetingClient:
    @staticmethod
    async def connect(
        addr: str,
        cert_path: str,
        server_name: str = "localhost",
        timeout_secs: int = 30
    ) -> 'GreetingClient':
        """Connect to Greeting service"""
        ...

    async def greet(self, request: GreetRequest) -> GreetResponse:
        """Call greet RPC method"""
        ...
```

### Server Module (`server.py`)

Base class for implementing services:

```python
class GreetingServer:
    """Implement this to create a Python Greeting service"""

    async def greet_impl(
        self,
        request: GreetRequest
    ) -> GreetResponse:
        """Implement this method"""
        raise NotImplementedError()

    async def serve(
        self,
        addr: str,
        cert_path: str,
        key_path: str
    ):
        """Start serving requests"""
        ...
```

## Command-Line Options

Python-specific options for `rpcnet-gen`:

```bash
rpcnet-gen --help
```

```
Generate RPC client and server code from service definitions

Options:
  -i, --input <INPUT>    Input .rpc file
  -o, --output <OUTPUT>  Output directory [default: src/generated]
      --python           Generate Python bindings
      --server-only      Generate only server code
      --client-only      Generate only client code
      --types-only       Generate only type definitions
```

**Python-specific behavior**:
- `--python` flag enables Python code generation
- Output structure is `<output>/<service_name>/` (snake_case)
- Generates Python package with `__init__.py`
- Types use Python dataclasses and type hints

## Use Cases

### 1. Python Client → Rust Service

**Most common**: Use Python for scripting/tooling while running high-performance Rust services.

```python
# Python client
from directorregistry import DirectorRegistryClient, GetWorkerRequest

director = await DirectorRegistryClient.connect("127.0.0.1:61000", ...)
worker_info = await director.get_worker(GetWorkerRequest(...))
```

**Benefits**:
- Rapid development in Python
- Production performance from Rust
- Type-safe API with auto-completion

### 2. Python Service → Rust Client

Implement services in Python for rapid prototyping or ML integration:

```python
from greeting import GreetingServer, GreetRequest, GreetResponse

class MyGreeter(GreetingServer):
    async def greet_impl(self, request: GreetRequest) -> GreetResponse:
        # Use Python ML libraries, etc.
        return GreetResponse(message=f"Hello, {request.name}!")

# Start service
server = MyGreeter()
await server.serve("0.0.0.0:50051", cert_path="...", key_path="...")
```

**Benefits**:
- Access Python ecosystem (ML, data processing)
- Rapid iteration during development
- Same protocol as Rust services

### 3. Polyglot Microservices

Mix Python and Rust services in a distributed system:

```
┌─────────────────┐
│  Rust Director  │  ← High performance coordinator
└────────┬────────┘
         │
    ┌────┴────┬──────────┐
    ▼         ▼          ▼
┌────────┐ ┌──────┐  ┌──────────┐
│Rust    │ │Python│  │Python ML │
│Worker  │ │Worker│  │Worker    │
└────────┘ └──────┘  └──────────┘
```

**Benefits**:
- Right tool for each job
- Unified RPC protocol
- Type-safe boundaries

## Real-World Example: Cluster Client

See `examples/python/cluster/` for a complete example demonstrating Python clients connecting to a Rust cluster.

### Prerequisites

```bash
# 1. Generate TLS certificates
mkdir -p certs && cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
cd ..

# 2. Build Python module
maturin develop --features python --release
```

### Start Rust Cluster

```bash
# Terminal 1 - Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Worker
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

### Run Python Client

```bash
python examples/python/cluster/python_client.py
```

**Output**:
```
====================================================================
Python Client for RpcNet Cluster - Director Connection Demo
====================================================================

1️⃣  Connecting to director registry...
   ✅ Connected to director at 127.0.0.1:61000

2️⃣  Requesting workers (testing load balancing)...
   Request 1:
      ✅ Worker: worker-a
      📍 Address: 127.0.0.1:62001
      🔗 Connection ID: conn-1234

✅ Python client completed successfully!
```

### Full Workflow Example

`python_streaming_client.py` demonstrates the complete flow:

1. Connect to director to get available worker
2. Connect to worker for inference
3. Send multiple inference requests
4. Test load balancing

```python
# 1. Get worker from director
director = await DirectorRegistryClient.connect("127.0.0.1:61000", ...)
worker_info = await director.get_worker(GetWorkerRequest(...))

# 2. Connect to worker
worker = await InferenceClient.connect(worker_info.worker_addr, ...)

# 3. Send inference request
response = await worker.infer(InferenceRequest(
    connection_id=worker_info.connection_id,
    prompt="Hello from Python!"
))
print(response.response)
```

## Features

### ✅ Type Safety

- Python dataclasses with type hints
- IDE auto-completion support
- Runtime type checking via dataclasses

```python
# Type-safe request construction
request = GreetRequest(name="Alice")  # ✅
request = GreetRequest(age=25)        # ❌ Type error
```

### ✅ Async/Await

- Native Python asyncio integration
- Non-blocking I/O
- Concurrent request handling

```python
# Parallel requests
responses = await asyncio.gather(
    client.greet(GreetRequest(name="Alice")),
    client.greet(GreetRequest(name="Bob")),
    client.greet(GreetRequest(name="Charlie")),
)
```

### ✅ Automatic Serialization

- MessagePack encoding/decoding (via `rmp-serde`)
- Handles complex nested types
- Fully compatible with Rust MessagePack serialization
- Cross-language type safety maintained

```python
# Automatic serialization
request = GreetRequest(name="Alice")
response = await client.greet(request)  # Serialized → sent → deserialized
```

### ✅ Error Handling

Service errors are raised as Python exceptions:

```python
try:
    response = await client.greet(request)
except GreetError.InvalidName as e:
    print(f"Invalid name: {e}")
except ConnectionError:
    print("Connection failed")
```

### ✅ Connection Management

- Automatic connection pooling
- Configurable timeouts
- TLS certificate verification

```python
client = await GreetingClient.connect(
    addr="127.0.0.1:50051",
    cert_path="certs/test_cert.pem",
    server_name="localhost",
    timeout_secs=30  # Configurable timeout
)
```

## Performance Considerations

### Serialization

- **MessagePack**: ~10-50µs overhead per call
- **Faster than JSON**: Binary format, compact encoding
- **Cross-language**: Python ↔ Rust compatibility

### Network

- **QUIC+TLS**: Same transport as Rust-to-Rust
- **Throughput**: 10K+ requests/sec from Python
- **Latency**: Minimal overhead (~100µs) vs native Rust

### Python Overhead

Python adds overhead compared to Rust:

| Aspect | Rust | Python |
|--------|------|--------|
| **CPU** | ⚡⚡⚡ | ⚡⚡ |
| **Latency** | ~1-10µs | ~10-50µs |
| **Throughput** | 100K+ req/s | 10K+ req/s |

**Recommendation**: Use Python for:
- Non-critical path operations
- Tooling and monitoring
- Rapid prototyping
- ML inference workloads

Use Rust for:
- Hot path / critical services
- High-throughput systems
- Low-latency requirements

## Streaming Support

### ✅ Bidirectional Streaming Supported

Python codegen **supports bidirectional streaming** RPCs using `AsyncIterable` and `AsyncIterator`:

**Rust Service Definition**:
```rust
use futures::Stream;
use std::pin::Pin;

#[rpcnet::service]
pub trait Inference {
    async fn generate(
        &self,
        request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>
    ) -> Result<Pin<Box<dyn Stream<Item = Result<InferenceResponse, InferenceError>> + Send>>, InferenceError>;
}
```

**Generated Python Client**:
```python
class InferenceClient:
    async def generate(
        self,
        request_stream: AsyncIterable[InferenceRequest]
    ) -> AsyncIterator[InferenceResponse]:
        """Streaming RPC method: generate"""
        ...
```

**Python Usage Example**:
```python
async def request_generator():
    """Generate streaming requests"""
    for i in range(10):
        yield InferenceRequest(
            connection_id="conn-123",
            prompt=f"Request {i}"
        )

# Send streaming requests and receive streaming responses
async for response in client.generate(request_generator()):
    print(f"Received: {response}")
```

### Current Limitations

- **Client-side streaming**: Fully supported (AsyncIterable input)
- **Server-side streaming**: Fully supported (AsyncIterator output)
- **Bidirectional streaming**: Fully supported (both AsyncIterable and AsyncIterator)
- **Python server implementation**: Generated but needs runtime testing

## Type Compatibility

### Rust-Only Types

Some Rust types don't have direct Python equivalents:

- `std::time::Duration` → Use integer milliseconds
- Custom enums with data → Use struct variants
- `Option<T>` → Use `Optional[T]`

**Best practice**: Keep `.rpc.rs` types simple and cross-language compatible.

### ✅ Enums with Associated Data (Fully Supported)

The Python codegen **fully supports** Rust enums with associated data (tagged unions) by generating Union types with dataclasses.

**Example Rust Definition:**
```rust
pub enum InferenceResponse {
    Connected { worker: String, connection_id: String },
    Token { text: String, sequence: u64 },
    Error { message: String },
    Done,
}
```

**Generated Python Code:**
```python
from dataclasses import dataclass
from typing import Union

@dataclass
class InferenceResponseConnected:
    worker: str
    connection_id: str

@dataclass
class InferenceResponseToken:
    text: str
    sequence: int

@dataclass
class InferenceResponseError:
    message: str

@dataclass
class InferenceResponseDone:
    pass

# Union type for all variants
InferenceResponse = Union[
    InferenceResponseConnected,
    InferenceResponseToken,
    InferenceResponseError,
    InferenceResponseDone
]

# Auto-generated deserializer handles MessagePack formats
def deserialize_inferenceresponse(data: Any) -> InferenceResponse:
    """Deserialize MessagePack data to InferenceResponse variant."""
    variant_name, variant_data = next(iter(data.items()))

    if variant_name == 'Connected':
        # Handles both dict and list formats from MessagePack
        if isinstance(variant_data, dict):
            return InferenceResponseConnected(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceResponseConnected(*variant_data)
    # ... other variants
```

**Python Usage (Type-Safe!):**
```python
from inference import InferenceClient, InferenceRequest
from inference.types import (
    InferenceResponseConnected,
    InferenceResponseToken,
    InferenceResponseError,
    InferenceResponseDone
)

# Send request and get properly typed response
async for response in client.generate(request_generator()):
    # Type checking with isinstance()
    if isinstance(response, InferenceResponseConnected):
        print(f"Connected to worker: {response.worker}")
        print(f"Connection ID: {response.connection_id}")

    elif isinstance(response, InferenceResponseToken):
        print(f"Token #{response.sequence}: {response.text}")

    elif isinstance(response, InferenceResponseError):
        print(f"Error: {response.message}")

    elif isinstance(response, InferenceResponseDone):
        print("Done!")
```

**Key Features:**

✅ **Type Safety**: Each variant is a separate dataclass with proper fields
✅ **IDE Support**: Full auto-completion for variant fields
✅ **Pattern Matching**: Use `isinstance()` for clean variant handling
✅ **MessagePack Compatible**: Handles both dict `{'Connected': {...}}` and list formats
✅ **Automatic Deserialization**: Generated client methods call deserializer automatically

**Example**:
See `examples/python/cluster/python_real_streaming_client.py` for a complete working example.

## Troubleshooting

### "Module not found: _rpcnet"

**Problem**: Python can't import the native module.

**Solution**: Build the native module:
```bash
maturin develop --features python --release
```

### "Unknown method: Service.method"

**Problem**: Python bindings don't match the running Rust service.

**Solution**: Ensure you're using the actual service definitions:
```bash
# Copy actual service definition
cp examples/cluster/director_registry.rpc.rs examples/python/cluster/

# Regenerate bindings
./target/release/rpcnet-gen \
  --input examples/python/cluster/director_registry.rpc.rs \
  --output examples/python/cluster/generated \
  --python
```

### "Connection refused"

**Problem**: Rust service isn't running.

**Solution**: Start the Rust service first:
```bash
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director
```

### "Certificate verification failed"

**Problem**: TLS certificates missing or invalid.

**Solution**: Generate test certificates:
```bash
mkdir -p certs && cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
```

### Type Mismatches

**Problem**: Request/response types don't match between Python and Rust.

**Solution**:
1. Ensure both use the same `.rpc.rs` file
2. Regenerate Python bindings after any Rust changes
3. Restart Python interpreter to reload modules

## Best Practices

### 1. Version Control Generated Code

**Option A - Commit generated code**:
```bash
# .gitignore
# (no ignore for generated/)
```

**Option B - Regenerate on demand**:
```bash
# .gitignore
generated/

# README.md
Run: rpcnet-gen --input service.rpc.rs --output generated --python
```

**Recommendation**: Commit for libraries, regenerate for applications.

### 2. Keep Service Definitions Simple

```rust
// ✅ Good - simple, cross-language types
#[derive(Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub count: i32,
    pub tags: Vec<String>,
}

// ❌ Avoid - Rust-specific types
pub struct Request {
    pub id: Uuid,                    // Not in Python
    pub timeout: Duration,           // Use i64 millis instead
    pub callback: Box<dyn Fn()>,     // Can't serialize
}
```

### 3. Document Your API

Add docstrings to generated code:

```python
# Manually enhance generated code with docs
class GreetingClient:
    async def greet(self, request: GreetRequest) -> GreetResponse:
        """
        Send a greeting request.

        Args:
            request: Request with name to greet

        Returns:
            Response with greeting message

        Raises:
            GreetError.InvalidName: If name is empty
        """
        ...
```

### 4. Handle Errors Gracefully

```python
async def safe_greet(client, name):
    try:
        response = await client.greet(GreetRequest(name=name))
        return response.message
    except GreetError.InvalidName:
        return "Invalid name provided"
    except ConnectionError:
        return "Service unavailable"
    except Exception as e:
        logger.error(f"Unexpected error: {e}")
        return "Error occurred"
```

### 5. Use Connection Pooling

```python
# ✅ Reuse client connections
client = await GreetingClient.connect(...)

for name in names:
    response = await client.greet(GreetRequest(name=name))

# ❌ Don't reconnect every time
for name in names:
    client = await GreetingClient.connect(...)  # Wasteful!
    response = await client.greet(GreetRequest(name=name))
```

## Next Steps

- **[Example Programs](reference/examples.md)** - See `examples/python/cluster/`
- **[rpcnet-gen CLI](rpcnet-gen.md)** - Full code generation documentation
- **[Cluster Example](cluster-example.md)** - Distributed systems with Python

## Complete Example Code

See the full working example at:
- **`examples/python/cluster/README.md`** - Complete usage guide
- **`examples/python/cluster/QUICKSTART.md`** - Quick start guide
- **`examples/python/cluster/python_client.py`** - Simple client example
- **`examples/python/cluster/python_streaming_client.py`** - Full workflow example

The Python cluster example demonstrates:
- ✅ Connecting to Rust director
- ✅ Getting available workers
- ✅ Sending inference requests
- ✅ Load balancing
- ✅ Error handling
- ✅ Type-safe Python API

Generate the bindings and try it yourself!
