# RpcNet Python Client/Server Example

High-performance RPC server with **true multi-process parallelism** - each worker has its own Python interpreter and GIL.

## Features

✅ **Type-safe handlers** - Implement typed methods from `.rpc.rs` definition  
✅ **Multi-process** - 11 worker processes by default (customizable)  
✅ **Auto-serialization** - Handler instances pickled and sent to workers  
✅ **15k+ req/s** - Single async client achieves 15,574 req/s  
✅ **Low latency** - 0.064ms average latency under load  

## Files

- `benchmark.rpc.rs` - Service definition (compile-time types)
- `server.py` - Multi-process server implementation (typed handlers)
- `client.py` - Blocking client (single-threaded baseline)
- `async_client.py` - Async client (maximum single-client throughput)
- `benchmark.py` - Multi-threaded benchmark (10 threads)
- `generated/` - Auto-generated type-safe client/server code

## Quick Start

### 1. Install Dependencies

```bash
pip install cloudpickle
```

**Required**: `cloudpickle` serializes handler instances to worker processes.

### 2. Generate Code

```bash
rpcnet-gen --input benchmark.rpc.rs --output generated --python
```

This generates:
- `generated/benchmarkservice/types.py` - Request/response types
- `generated/benchmarkservice/client.py` - Type-safe client
- `generated/benchmarkservice/server.py` - Type-safe server base class

### 3. Start Server

```bash
# Default: 11 worker processes
python server.py

# Custom worker count (use CPU count for max throughput)
PROCESSES=16 python server.py

# Bind to different address
BIND_ADDR=0.0.0.0:8080 python server.py
```

### 4. Run Benchmarks

Open a new terminal and run:

```bash
# Baseline: Single-threaded blocking client
python client.py
# Expected: ~7,800 req/s

# Maximum single-client throughput
python async_client.py
# Expected: ~15,600 req/s (10,000 concurrent requests)

# Multi-threaded stress test
python benchmark.py
# Expected: ~17,000 req/s (10 threads × 1,000 req each)
```

## Performance Tuning

### Maximum Throughput Configuration

For maximum throughput, match worker count to available CPU cores:

```bash
# Check CPU count
python -c "import os; print(os.cpu_count())"

# Start server with optimal workers
PROCESSES=$(python -c "import os; print(os.cpu_count())") python server.py
```

**Benchmark Results** (M1 MacBook, 11 workers):

| Client Type | Throughput | Latency (avg) | Concurrency |
|-------------|-----------|---------------|-------------|
| Blocking (single-threaded) | 7,800 req/s | 0.128 ms | 1 |
| Async (fully concurrent) | **15,600 req/s** | **0.064 ms** | 10,000 |
| Multi-threaded (10 threads) | 17,200 req/s | 0.058 ms | 10,000 |

### Server Configuration

**Environment Variables:**

- `PROCESSES` - Number of worker processes (default: 11)
  - Set to CPU count for maximum throughput
  - Each worker has its own Python GIL
  
- `BIND_ADDR` - Server bind address (default: `127.0.0.1:50051`)
  - Use `0.0.0.0:PORT` to accept remote connections
  
Example:
```bash
PROCESSES=16 BIND_ADDR=0.0.0.0:50051 python server.py
```

### Client Optimization

**Async Client** (best single-client throughput):
- Launches all requests concurrently
- Minimal overhead from single event loop
- Best for: Maximum throughput from one process

**Blocking Client** (baseline):
- Single-threaded, sequential requests
- Best for: Simple use cases, testing

**Multi-threaded Benchmark**:
- Multiple OS threads, each with blocking client
- Best for: Simulating multiple concurrent clients

## Architecture

```
┌─────────────────────────────────────────┐
│         RpcNet Multi-Process Server      │
├─────────────────────────────────────────┤
│  Master Process (Async Event Loop)      │
│  ├─ TLS Listener (127.0.0.1:50051)     │
│  ├─ Round-robin request routing         │
│  └─ Worker management                   │
├─────────────────────────────────────────┤
│  Worker 0  │  Worker 1  │ ... │ Worker 10│
│  (Own GIL) │  (Own GIL) │     │ (Own GIL)│
│  Unix sock │  Unix sock │     │ Unix sock│
└─────────────────────────────────────────┘
          ↑
    Client requests (TLS/QUIC)
```

**How It Works:**

1. **Master process** accepts client connections via TLS
2. **Request routing** - Round-robin across worker processes
3. **Worker processing** - Each worker:
   - Runs in separate OS process (own Python GIL)
   - Executes handler methods (typed, auto-serialized)
   - Communicates via Unix domain sockets
4. **Response** - Routed back to client via master

**Key Benefits:**
- ✅ No GIL contention (true parallelism)
- ✅ Full CPU utilization across all cores
- ✅ Fault isolation (worker crash doesn't kill server)
- ✅ Type safety from `.rpc.rs` to Python handlers

## Implementation Example

See `server.py` for the full implementation. Here's the pattern:

```python
import rpcnet
from benchmarkservice import BenchmarkServiceHandler, BenchmarkServiceServer
from benchmarkservice.types import *

# 1. Implement typed handler (gets auto-pickled to workers)
class MyBenchmarkHandler(BenchmarkServiceHandler):
    async def noop(self, request: NoopRequest) -> NoopResponse:
        return NoopResponse(success=True)
    
    async def process(self, request: BenchmarkRequest) -> BenchmarkResponse:
        return BenchmarkResponse(
            echo=request.message,
            doubled=request.value * 2,
            server_time_ns=time.time_ns()
        )

# 2. Create server with handler
async def main():
    config = rpcnet.RpcConfig(
        cert_path="../../../certs/test_cert.pem",
        bind_addr="127.0.0.1:50051",
        key_path="../../../certs/test_key.pem",
    )
    
    handler = MyBenchmarkHandler()
    server = BenchmarkServiceServer(handler, config)
    await server.serve()  # Blocks until shutdown

asyncio.run(main())
```

**Type Safety:**
- Request/response types match `.rpc.rs` definition
- IDE autocomplete for all fields
- Compile-time type checking with mypy/pyright
- Handler methods must match exact signatures

## Troubleshooting

**"ModuleNotFoundError: No module named 'cloudpickle'"**
```bash
pip install cloudpickle
```

**"Address already in use"**
```bash
# Kill existing server
pkill -9 -f "python.*server.py"

# Or use different port
BIND_ADDR=127.0.0.1:50052 python server.py
```

**Low throughput**
- Check worker count matches CPU cores: `PROCESSES=$(nproc) python server.py`
- Ensure client uses async/concurrent requests (not sequential)
- Verify no CPU throttling (check `top` or Activity Monitor)

**"cloudpickle=false" in logs**
- `cloudpickle` not installed or wrong Python environment
- Install in same virtualenv as server: `pip install cloudpickle`
