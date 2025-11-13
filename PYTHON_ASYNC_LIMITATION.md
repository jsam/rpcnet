# Python Async Handler Implementation

## Summary

✅ **RESOLVED** - Python async server handlers are now fully functional with high performance! The issue has been resolved by implementing a **persistent event loop thread architecture** that efficiently bridges Tokio and asyncio.

**Performance**: ~4,600 calls/sec throughput with sub-millisecond latency (0.22 ms/call)

## What Works ✅

- **Python RPC Clients**: Fully functional, can call Rust servers
- **Generated Python client code**: Works perfectly with asyncio
- **Python RPC Servers**: ✅ **NOW WORKING** - Server creation and handler registration fully functional
- **Python async server handlers**: ✅ **NOW WORKING** - Can register and invoke async handlers with high performance
- **Persistent Event Loop**: ✅ **NEW** - Dedicated thread with reused asyncio event loop for optimal performance
- **Serialization**: MessagePack serialization works for all dict/struct types
- **Examples**: `examples/python/cluster/python_client.py` demonstrates working client usage
- **Benchmarks**: `benches/python_event_loop_bench.py` demonstrates performance characteristics

## Historical Context: What Didn't Work ❌

Previously, the following limitations existed:

- **Python async server handlers**: Could not be registered due to "no running event loop" error
- **Python RPC servers**: Server creation worked, but registering async handlers failed
- **Integration tests**: Tests that required Python servers failed

## Solution Implemented ✅

### Persistent Event Loop Thread Architecture

The solution uses a **persistent event loop thread** with a `PythonEventLoopExecutor` that maintains a dedicated asyncio event loop for the lifetime of the executor. This provides optimal performance by reusing the event loop across all handler invocations.

**Architecture** (`src/python/event_loop.rs`):

1. **Dedicated OS Thread**: A single OS thread is spawned at executor creation time
2. **Persistent asyncio Event Loop**: The thread creates and maintains one asyncio event loop via `asyncio.new_event_loop()`
3. **Channel-Based Communication**: Uses `tokio::sync::mpsc` for requests and `oneshot` for responses
4. **GIL Management**: Critical optimization - GIL is released while waiting for requests, preventing deadlocks

**Implementation**:
```rust
pub struct PythonEventLoopExecutor {
    request_tx: Arc<mpsc::UnboundedSender<ExecutionRequest>>,
}

impl PythonEventLoopExecutor {
    pub fn new() -> Result<Self, String> {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel();

        // Spawn dedicated event loop thread (once per executor)
        std::thread::spawn(move || {
            // Create event loop once
            let event_loop = Python::with_gil(|py| {
                let asyncio = py.import("asyncio")?;
                let new_loop = asyncio.call_method0("new_event_loop")?;
                asyncio.call_method1("set_event_loop", (&new_loop,))?;
                Ok(new_loop.unbind())
            });

            loop {
                // Wait for request WITHOUT holding GIL (critical!)
                let request = request_rx.blocking_recv();

                // Execute handler with GIL
                let result = Python::with_gil(|py| {
                    let event_loop = event_loop.bind(py);
                    let coroutine = handler.call1(py, (params_bytes,))?;
                    event_loop.call_method1("run_until_complete", (coroutine,))
                });

                let _ = response_tx.send(result);
            }
        });

        Ok(Self { request_tx: Arc::new(request_tx) })
    }
}
```

**Key advantages:**
1. **~21x faster** than per-invocation event loop creation (~4,600 calls/sec vs ~220 calls/sec)
2. **Sub-millisecond latency**: 0.22 ms average per call
3. **Reuses event loop**: No setup/teardown overhead per invocation
4. **GIL optimization**: Releases GIL while waiting, preventing main thread deadlocks
5. **Channel-based**: Clean separation between Tokio and asyncio contexts
6. **Works with any Python async function** that uses `await`

**Example usage:**
```python
import asyncio
from _rpcnet import RpcServer, RpcConfig

# Create server
config = RpcConfig(
    cert_path="certs/cert.pem",
    key_path="certs/key.pem",
    bind_addr="127.0.0.1:8080",
    server_name="localhost"
)
server = RpcServer(config)

# Define async handler
async def my_handler(request_bytes: bytes) -> bytes:
    # Can use await, asyncio operations, etc.
    await asyncio.sleep(0.01)
    return process_request(request_bytes)

# Register and serve
await server.register("my_method", my_handler)
await server.serve()
```

## Historical Technical Details

### The Original Problem

When registering a Python async handler with the Rust RPC server:

```python
async def my_handler(request_bytes: bytes) -> bytes:
    # Process request
    return response_bytes

await server.register("my_method", my_handler)
```

The handler invocation fails with:
```
RuntimeError: no running event loop
```

### Root Cause

The issue occurs in `src/python/server.rs` when converting a Python coroutine to a Rust future:

```rust
// This line fails:
pyo3_async_runtimes::tokio::into_future(coroutine.into_bound(py))
```

The problem is that `into_future()` requires access to a Python event loop, but when the handler is invoked (from within the Rust/Tokio async runtime), there's no Python event loop in that context.

### Why It's Hard to Fix

1. **Event Loop Context Mismatch**: The Rust handler runs in a Tokio context, while Python async requires an asyncio event loop
2. **pyo3-async-runtimes Limitations**: The `TaskLocals` pattern doesn't fully bridge the gap when calling Python async from Rust async in a spawned task context
3. **Send/Sync Boundaries**: `scope_local()` returns `!Send` futures which can't be used in the RPC server's multi-threaded context

### Attempted Solutions

We tried multiple approaches:

1. **Using `scope()` with Task Locals** - Still no event loop access
2. **Using `scope_local()` with `block_on()`** - Violates `Send` boundary requirements
3. **Capturing event loop at registration time** - Event loop not available during handler execution

## Workaround

For now, Python bindings should be used **client-side only**:

```python
# ✅ This works - Python client calling Rust server
from generated.registry import RegistryClient, GetWorkerRequest

client = await RegistryClient.connect("127.0.0.1:8080", cert_path="cert.pem")
response = await client.my_method(request)
```

## Alternative Approaches Considered

During the implementation, several approaches were evaluated:

1. ✅ **Persistent Event Loop Thread with Channels** (CURRENT IMPLEMENTATION)
   - Maintains a dedicated Python event loop in a separate thread
   - Channel-based request/response communication
   - Releases GIL while waiting for requests (critical for preventing deadlocks)
   - **Best performance**: ~4,600 calls/sec with 0.22 ms latency
   - **Status**: Fully implemented and tested

2. ⚠️  **Run Python handlers with `spawn_blocking` + `asyncio.run()`** (DEPRECATED)
   - Creates fresh event loop for each handler invocation
   - Clean separation between Tokio and asyncio contexts
   - Simple and reliable but ~21x slower than persistent thread approach
   - **Performance**: ~220 calls/sec (superseded by persistent thread)
   - **Status**: Replaced by persistent event loop thread

3. ❌ **Use synchronous Python handlers**
   - Less idiomatic for Python async code
   - Doesn't provide the async/await experience users expect

4. ❌ **pyo3-async-runtimes TaskLocals pattern**
   - Attempted but still had "no running event loop" errors
   - `scope_local()` returns `!Send` futures incompatible with multi-threaded server

## Future Work

### ✅ Completed Optimizations

1. **✅ Persistent Event Loop Thread** - Implemented and tested
   - Dedicated OS thread with reused asyncio event loop
   - ~21x performance improvement over per-invocation approach
   - See `src/python/event_loop.rs` for implementation

2. **✅ Event Loop Context Reuse** - Implemented
   - Single event loop maintained for executor lifetime
   - GIL management optimized to prevent deadlocks

### 🔮 Future Enhancements

1. **Async Streaming Support** ⭐ **Next Priority**
   - Extend the persistent event loop pattern to support streaming handlers
   - Server streaming (1→N), client streaming (N→1), bidirectional (N→M)
   - Design document: `docs/PYTHON_STREAMING_DESIGN.md`
   - Estimated timeline: 9-13 days for full implementation

2. **Concurrent Handler Execution**
   - Currently handlers execute sequentially due to Python's GIL
   - Could explore multi-process architecture for true parallelism
   - Would require inter-process communication overhead

3. **Backpressure Management**
   - Add bounded channels with configurable buffer sizes
   - Implement flow control for high-throughput scenarios

## Performance Benchmarks

**Benchmark**: `benches/python_event_loop_bench.py`

Results from persistent event loop thread implementation:

```
RESULTS:
Total calls:        500
Total time:         0.109 seconds
Avg latency:        0.22 ms/call
Throughput:         4,593 calls/sec

Latency by payload size:
     10 bytes:   0.17 ms/call
    100 bytes:   0.18 ms/call
   1024 bytes:   0.20 ms/call
  10240 bytes:   0.67 ms/call
```

**Performance Comparison**:
- **Old approach** (`spawn_blocking` + `asyncio.run()`): ~220 calls/sec
- **New approach** (persistent event loop thread): ~4,600 calls/sec
- **Improvement**: **~21x faster** 🚀

**Key Insight**: The persistent event loop eliminates the overhead of creating and destroying an event loop for each handler invocation.

## Testing Impact

**Current Status**: All Python async server handler tests now pass! ✅

**Passing Tests**:
- ✅ Persistent event loop executor creation tests
- ✅ Event loop thread initialization and cleanup tests
- ✅ Simple async handler execution tests
- ✅ Async handlers with `await asyncio.sleep()` and async operations
- ✅ Executor reuse across multiple handlers
- ✅ End-to-end Python server integration tests
- ✅ All serialization tests for dicts/structs
- ✅ Config creation tests
- ✅ Client tests
- ✅ MessagePack roundtrip tests

**Previously Failing (Now Fixed)**:
- ✅ Tests requiring Python async server handlers
- ✅ Integration tests with Python servers
- ✅ GIL deadlock issue when using `asyncio.run()` after server creation

**Note**: Streaming handlers still need dedicated implementation (design document available)

## Related Issues

- PyO3/pyo3-async-runtimes#100: "RuntimeError: no running event loop"
- PyO3/pyo3-async-runtimes#105: Basic example with same error
- StackOverflow: "Rust PyO3-asyncio; awaiting Python coroutine in spawned tokio::task"

## Critical Implementation Details

### GIL Management (Essential for Correctness)

The most critical aspect of the persistent event loop thread implementation is **GIL release management**:

**❌ Incorrect (causes deadlock)**:
```rust
Python::with_gil(|py| {
    loop {
        let request = request_rx.blocking_recv(); // GIL held!
        execute_handler(py, request);
    }
});
```

**✅ Correct (releases GIL between requests)**:
```rust
loop {
    let request = request_rx.blocking_recv(); // GIL released!

    let result = Python::with_gil(|py| {
        execute_handler(py, request)
    }); // GIL released again after handler completes
}
```

**Why This Matters**:
- The event loop thread waits for requests most of the time
- If the GIL is held during this wait, **no other thread can use Python**
- Main thread calling `asyncio.run()` will deadlock waiting for GIL
- Releasing GIL between handler invocations allows concurrent Python access

This is documented in `src/python/event_loop.rs:62-63`.

### Sequential Processing

Python async handlers process **sequentially**, not concurrently, due to the Global Interpreter Lock (GIL):
- One handler executes at a time in the event loop thread
- This is **expected behavior**, not a limitation
- For concurrent execution, consider multi-process architecture (future work)

## Conclusion

✅ **The Python bindings are now production-ready for both client and server use!**

Key capabilities:
- ✅ **Python RPC Clients**: Fully functional, can call any RPC server
- ✅ **Python RPC Servers**: Fully functional with high-performance async handler support
- ✅ **Async/Await Support**: Python handlers can use `await`, `asyncio.sleep()`, and all async patterns
- ✅ **Performance**: ~4,600 calls/sec throughput with sub-millisecond latency
- ✅ **Type Safety**: Full MessagePack serialization for complex types
- ✅ **End-to-End Testing**: Comprehensive test suite validates server functionality
- ✅ **Persistent Event Loop**: Optimized architecture with proper GIL management

This solution is suitable for:
- **Polyglot microservices**: Mix Python and Rust services seamlessly
- **Rapid prototyping**: Build servers in Python, migrate to Rust for performance
- **Testing**: Write Python servers for testing Rust clients
- **Scripting**: Python servers for automation and tooling
- **Production workloads**: Performance characteristics suitable for real-world use

**Performance Note**: Python servers now achieve ~4,600 calls/sec with sub-ms latency, making them viable for many production scenarios. For extreme performance requirements (>10k calls/sec), Rust servers are still recommended.
