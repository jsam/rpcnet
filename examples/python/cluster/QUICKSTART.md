# Quick Start - Python Cluster Example

## TL;DR

```bash
# 1. Generate Python bindings (already done)
ls generated/compute generated/registry

# 2. Build Python module
source .venv/bin/activate
maturin develop --features python --release

# 3. Start Rust cluster
# Terminal 1
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2
WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# 4. Run Python client
cd examples/python/cluster
python python_client.py
```

## What You'll See

```
====================================================================
Python Client for RpcNet Cluster
====================================================================

📁 Using certificate: ../../certs/test_cert.pem
🎯 Director address: 127.0.0.1:61000

1️⃣  Connecting to director...
   ✅ Connected to director at 127.0.0.1:61000

2️⃣  Requesting available worker...
   ✅ Got worker: worker-a
   📍 Address: 127.0.0.1:62001

3️⃣  Connecting to worker...
   ✅ Connected to worker

4️⃣  Sending compute tasks...
   📤 Sending task: task-1
   📥 Result: Processed: Process this data
      Worker: worker-a

   ...

✅ Python client completed successfully!
```

## How It Works

### 1. Service Definition (Rust)

```rust
// compute.rpc.rs
#[rpcnet::service]
pub trait Compute {
    async fn process(&self, request: ComputeRequest)
        -> Result<ComputeResponse, ComputeError>;
}
```

### 2. Generate Python Code

```bash
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/python/cluster/compute.rpc.rs \
  --output examples/python/cluster/generated \
  --python
```

### 3. Use in Python

```python
from generated.compute import ComputeClient, ComputeRequest

# Connect
client = await ComputeClient.connect(
    "127.0.0.1:62001",
    cert_path="certs/test_cert.pem"
)

# Call
response = await client.process(
    ComputeRequest(task_id="1", data="test")
)
print(response.result)
```

## Files Generated

```
generated/
├── compute/
│   ├── types.py      ← ComputeRequest, ComputeResponse
│   ├── client.py     ← ComputeClient
│   └── server.py     ← ComputeServer (to implement)
│
└── registry/
    ├── types.py      ← GetWorkerRequest, GetWorkerResponse
    ├── client.py     ← RegistryClient
    └── server.py     ← RegistryServer (to implement)
```

## Full Example

See `python_client.py` for complete working code:

```python
import asyncio
from generated.registry import RegistryClient, GetWorkerRequest
from generated.compute import ComputeClient, ComputeRequest

async def main():
    # Get worker from director
    director = await RegistryClient.connect("127.0.0.1:61000", ...)
    worker_info = await director.get_worker(GetWorkerRequest(...))

    # Connect to worker
    worker = await ComputeClient.connect(worker_info.worker_addr, ...)

    # Send task
    response = await worker.process(ComputeRequest(...))
    print(response.result)

asyncio.run(main())
```

## Troubleshooting

### "Module not found: _rpcnet"

Build the Python module:
```bash
maturin develop --features python --release
```

### "Connection refused"

Start the Rust cluster first:
```bash
# Terminal 1 - Director
DIRECTOR_ADDR=127.0.0.1:61000 cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Worker
WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 \
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
