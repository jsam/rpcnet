# Quick Start - Python Cluster Example

## TL;DR

```bash
# 1. Generate TLS certificates (if needed)
mkdir -p certs && cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
  -days 365 -nodes -subj "/CN=localhost"
cd ..

# 2. Build Python module
maturin develop --features python --release

# 3. Start Rust cluster (3 terminals)
# Terminal 1 - Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Worker A
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# 4. Run Python client (from project root)
python examples/python/cluster/python_client.py

# Or run the full workflow demo
python examples/python/cluster/python_streaming_client.py
```

## What You'll See

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

   ...

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

...

✅ Python Streaming Client Demo Completed Successfully!
```

## How It Works

### 1. Service Definition (Rust)

The actual running cluster uses these services:

```rust
// director_registry.rpc.rs (from examples/cluster/)
#[rpcnet::service]
pub trait DirectorRegistry {
    async fn get_worker(&self, request: GetWorkerRequest)
        -> Result<GetWorkerResponse, DirectorError>;
}

// inference.rpc.rs (from examples/cluster/)
#[rpcnet::service]
pub trait Inference {
    async fn infer(&self, request: InferenceRequest)
        -> Result<InferenceResponse, InferenceError>;
}
```

### 2. Generate Python Code

```bash
# Build code generator
cargo build --release --bin rpcnet-gen --features codegen,python

# Generate bindings (matching actual services)
./target/release/rpcnet-gen \
  --input examples/python/cluster/director_registry.rpc.rs \
  --output examples/python/cluster/generated \
  --python

./target/release/rpcnet-gen \
  --input examples/python/cluster/inference.rpc.rs \
  --output examples/python/cluster/generated \
  --python
```

### 3. Use in Python

```python
from directorregistry import DirectorRegistryClient, GetWorkerRequest
from inference import InferenceClient, InferenceRequest

# Connect to director
director = await DirectorRegistryClient.connect(
    "127.0.0.1:61000",
    cert_path="../../../certs/test_cert.pem"
)

# Get worker
worker_info = await director.get_worker(
    GetWorkerRequest(connection_id=None, prompt="test")
)

# Connect to worker
worker = await InferenceClient.connect(
    worker_info.worker_addr,
    cert_path="../../../certs/test_cert.pem"
)

# Send inference request
response = await worker.infer(
    InferenceRequest(
        connection_id=worker_info.connection_id,
        prompt="Hello!"
    )
)
print(response.response)
```

## Files Generated

```
generated/
├── directorregistry/        # Director service bindings
│   ├── __init__.py
│   ├── types.py             ← GetWorkerRequest, GetWorkerResponse, DirectorError
│   ├── client.py            ← DirectorRegistryClient
│   └── server.py            ← DirectorRegistryServer
│
└── inference/               # Worker service bindings
    ├── __init__.py
    ├── types.py             ← InferenceRequest, InferenceResponse, InferenceError
    ├── client.py            ← InferenceClient
    └── server.py            ← InferenceServer
```

##Full Example

See `python_streaming_client.py` for complete working code:

```python
import asyncio
from directorregistry import DirectorRegistryClient, GetWorkerRequest
from inference import InferenceClient, InferenceRequest

async def main():
    # 1. Get worker from director
    director = await DirectorRegistryClient.connect("127.0.0.1:61000", ...)
    worker_info = await director.get_worker(GetWorkerRequest(...))

    # 2. Connect to worker
    worker = await InferenceClient.connect(worker_info.worker_addr, ...)

    # 3. Send inference request
    response = await worker.infer(InferenceRequest(...))
    print(response.response)

asyncio.run(main())
```

## Troubleshooting

### "Module not found: _rpcnet"

Build the Python module:
```bash
maturin develop --features python --release
```

### "Connection refused" or "Unknown method"

Start the Rust cluster first (must be running before Python clients):
```bash
# Terminal 1 - Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Worker
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker
```

### "Certificate not found"

Generate test certificates:
```bash
mkdir -p certs
cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \
  -out test_cert.pem -days 365 -nodes -subj "/CN=localhost"
```

## Next Steps

1. **Read** `README.md` for detailed documentation
2. **Examine** generated code in `generated/`
3. **Modify** `python_client.py` to experiment
4. **Implement** your own Python worker using `ComputeServer`

## Summary

- ✅ Python bindings generated from Rust service definitions
- ✅ Type-safe async Python API
- ✅ Full example showing Python ↔ Rust RPC
- ✅ Ready to use!

**Time to working example**: ~2 minutes 🚀
