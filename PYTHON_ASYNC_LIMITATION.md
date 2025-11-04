# Python Async Handler Limitation

## Summary

The Python bindings for RpcNet currently have a limitation with async server-side handlers due to PyO3 event loop integration issues.

## What Works ✅

- **Python RPC Clients**: Fully functional, can call Rust servers
- **Generated Python client code**: Works perfectly with asyncio
- **Serialization**: MessagePack serialization works for all dict/struct types
- **Examples**: `examples/python/cluster/python_client.py` demonstrates working client usage

## What Doesn't Work ❌

- **Python async server handlers**: Cannot be registered due to "no running event loop" error
- **Python RPC servers**: Server creation works, but registering async handlers fails
- **Integration tests**: Tests that require Python servers fail

## Technical Details

### The Problem

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

## Future Work

Potential solutions:

1. **Run Python handlers in a dedicated asyncio event loop thread**
   - Create a Python event loop in a separate thread
   - Bridge between Tokio and asyncio via channels
   - More complex but would fully support Python servers

2. **Use synchronous Python handlers**
   - Change the API to accept sync functions that return futures
   - Less idiomatic but might be easier to bridge

3. **Wait for pyo3-async-runtimes improvements**
   - The library is actively developed
   - Future versions may provide better patterns for this use case

## Testing Impact

**Passing Tests** (23/43):
- All serialization tests for dicts/structs
- Config creation tests
- Simple client tests
- MessagePack roundtrip tests

**Failing Tests** (20/43):
- Any test requiring Python async server handlers
- Integration tests with Python servers
- Streaming tests (require server-side handlers)

## Related Issues

- PyO3/pyo3-async-runtimes#100: "RuntimeError: no running event loop"
- PyO3/pyo3-async-runtimes#105: Basic example with same error
- StackOverflow: "Rust PyO3-asyncio; awaiting Python coroutine in spawned tokio::task"

## Conclusion

The Python bindings are **production-ready for client use** but **not yet suitable for server implementations**. This is acceptable for most use cases where:

- High-performance servers are written in Rust
- Python is used for clients, tools, and scripting
- The cluster example demonstrates this pattern effectively

For users who need Python servers, they should use the Rust implementation or wait for the async handler support to be resolved.
