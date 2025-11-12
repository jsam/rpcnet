# Python Async Handler Implementation

## Summary

✅ **RESOLVED** - Python async server handlers are now fully functional! The issue has been resolved by implementing a dedicated event loop executor that bridges Tokio and asyncio using `spawn_blocking` with `asyncio.run()`.

## What Works ✅

- **Python RPC Clients**: Fully functional, can call Rust servers
- **Generated Python client code**: Works perfectly with asyncio
- **Python RPC Servers**: ✅ **NOW WORKING** - Server creation and handler registration fully functional
- **Python async server handlers**: ✅ **NOW WORKING** - Can register and invoke async handlers
- **Serialization**: MessagePack serialization works for all dict/struct types
- **Examples**: `examples/python/cluster/python_client.py` demonstrates working client usage

## Historical Context: What Didn't Work ❌

Previously, the following limitations existed:

- **Python async server handlers**: Could not be registered due to "no running event loop" error
- **Python RPC servers**: Server creation worked, but registering async handlers failed
- **Integration tests**: Tests that required Python servers failed

## Solution Implemented ✅

### Event Loop Executor Pattern

The solution uses a `PythonEventLoopExecutor` that bridges Tokio and asyncio by running Python handlers in a thread pool with `tokio::task::spawn_blocking`:

**Implementation** (`src/python/event_loop.rs`):
```rust
pub async fn execute_handler(
    &self,
    handler: PyObject,
    params: Vec<u8>,
) -> Result<Vec<u8>, crate::RpcError> {
    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let asyncio = py.import("asyncio")?;
            let params_bytes = PyBytes::new(py, &params);
            let coroutine = handler.call1(py, (params_bytes,))?;

            // Run the coroutine using asyncio.run()
            // This creates a new event loop, runs the coroutine, and cleans up
            let result = asyncio.call_method1("run", (coroutine,))?;
            result.extract::<Vec<u8>>()
        })
    })
    .await?
}
```

**Key advantages:**
1. Each handler execution gets a fresh asyncio event loop via `asyncio.run()`
2. Runs in thread pool via `spawn_blocking`, avoiding Tokio context conflicts
3. No manual event loop management required
4. Works with any Python async function that uses `await`

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

1. ✅ **Run Python handlers with `spawn_blocking` + `asyncio.run()`** (IMPLEMENTED)
   - Creates fresh event loop for each handler invocation
   - Clean separation between Tokio and asyncio contexts
   - Simple and reliable

2. ❌ **Dedicated asyncio event loop thread with channels**
   - Would maintain a persistent Python event loop in a separate thread
   - More complex; requires channel-based bridging
   - May be worth exploring for performance optimization

3. ❌ **Use synchronous Python handlers**
   - Less idiomatic for Python async code
   - Doesn't provide the async/await experience users expect

4. ❌ **pyo3-async-runtimes TaskLocals pattern**
   - Attempted but still had "no running event loop" errors
   - `scope_local()` returns `!Send` futures incompatible with multi-threaded server

## Future Optimizations

Possible improvements:

1. **Persistent Event Loop Thread**: Maintain a dedicated Python event loop thread instead of creating one per invocation
2. **Connection Pooling**: Reuse event loop contexts for better performance
3. **Async Streaming Support**: Extend the pattern to support streaming handlers

## Testing Impact

**Current Status**: All Python async server handler tests now pass! ✅

**Passing Tests**:
- ✅ Event loop executor creation tests
- ✅ Simple async handler execution tests
- ✅ Async handlers with `await asyncio.sleep()` and async operations
- ✅ End-to-end Python server integration tests
- ✅ All serialization tests for dicts/structs
- ✅ Config creation tests
- ✅ Client tests
- ✅ MessagePack roundtrip tests

**Previously Failing (Now Fixed)**:
- ✅ Tests requiring Python async server handlers
- ✅ Integration tests with Python servers
- Note: Streaming handlers still need dedicated implementation

## Related Issues

- PyO3/pyo3-async-runtimes#100: "RuntimeError: no running event loop"
- PyO3/pyo3-async-runtimes#105: Basic example with same error
- StackOverflow: "Rust PyO3-asyncio; awaiting Python coroutine in spawned tokio::task"

## Conclusion

✅ **The Python bindings are now production-ready for both client and server use!**

Key capabilities:
- ✅ **Python RPC Clients**: Fully functional, can call any RPC server
- ✅ **Python RPC Servers**: Fully functional with async handler support
- ✅ **Async/Await Support**: Python handlers can use `await`, `asyncio.sleep()`, and all async patterns
- ✅ **Type Safety**: Full MessagePack serialization for complex types
- ✅ **End-to-End Testing**: Comprehensive test suite validates server functionality

This solution is suitable for:
- **Polyglot microservices**: Mix Python and Rust services seamlessly
- **Rapid prototyping**: Build servers in Python, migrate to Rust for performance
- **Testing**: Write Python servers for testing Rust clients
- **Scripting**: Python servers for automation and tooling

**Note**: For maximum performance, Rust servers are still recommended, but Python servers are now a viable option for many use cases.
