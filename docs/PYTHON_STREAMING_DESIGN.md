# Python Streaming Handler Support - Design Document

**Status:** 📋 Design Phase - Future Work
**Created:** 2025-11-13
**Related:** Persistent Event Loop Thread Implementation (Completed)

## Overview

This document outlines the design for extending the Python async handler support to include **streaming RPC handlers** (server streaming, client streaming, and bidirectional streaming).

## Current State

### ✅ What Works (Completed)

**Unary RPC Handlers:**
- Python async handlers for unary (single request → single response) RPCs
- Persistent event loop thread architecture
- Channel-based request/response communication
- GIL management for concurrent execution
- Performance: ~4,600 calls/sec with sub-ms latency

**Example:**
```python
async def my_handler(request_bytes: bytes) -> bytes:
    # Process request
    return response_bytes

await server.register("my_method", my_handler)
```

### ❌ What Doesn't Work Yet

**Streaming RPC Handlers:**
- Server streaming (1 request → N responses)
- Client streaming (N requests → 1 response)
- Bidirectional streaming (N requests → M responses)

## Design Goals

1. **Pythonic API**: Use async generators for natural streaming
2. **Reuse Architecture**: Extend existing persistent event loop executor
3. **Performance**: Maintain sub-ms latency per message
4. **Type Safety**: Clear contracts for stream direction

## Proposed API

### Server Streaming (1→N)

**Python Handler:**
```python
async def server_stream_handler(request_bytes: bytes):
    """Server streaming: 1 request → N responses"""
    # Yield multiple responses
    for i in range(10):
        yield response_bytes
        await asyncio.sleep(0.01)

await server.register_server_streaming("stream_method", server_stream_handler)
```

**Client Usage:**
```python
stream = await client.call_server_streaming("stream_method", request_bytes)
async for response in stream:
    process(response)
```

### Client Streaming (N→1)

**Python Handler:**
```python
async def client_stream_handler(request_stream):
    """Client streaming: N requests → 1 response"""
    total = 0
    async for request_bytes in request_stream:
        total += len(request_bytes)
    return f"Total: {total}".encode()

await server.register_client_streaming("upload", client_stream_handler)
```

**Client Usage:**
```python
async def request_generator():
    for i in range(10):
        yield data_chunk

response = await client.call_client_streaming("upload", request_generator())
```

### Bidirectional Streaming (N→M)

**Python Handler:**
```python
async def bidi_handler(request_stream):
    """Bidirectional streaming: N requests → M responses"""
    async for request_bytes in request_stream:
        # Process and yield response
        yield process(request_bytes)

await server.register_bidirectional("chat", bidi_handler)
```

**Client Usage:**
```python
async def send_messages():
    for msg in messages:
        yield msg

stream = await client.call_bidirectional("chat", send_messages())
async for response in stream:
    print(response)
```

## Architecture Design

### Challenge: Bridging Python Async Generators to Rust Streams

Python async generators (`async def` with `yield`) need to be bridged to Rust's `Stream` trait.

### Proposed Solution: Dual-Channel Communication

**For Server Streaming:**
```rust
// Request: Single message from Python to Rust
// Response: Stream of messages from Rust to Python

pub async fn execute_server_streaming_handler(
    &self,
    handler: PyObject,
    request: Vec<u8>,
) -> Result<impl Stream<Item = Vec<u8>>, RpcError> {
    let (tx, rx) = mpsc::unbounded_channel();

    // Spawn task to pull from Python async generator
    tokio::spawn(async move {
        Python::with_gil(|py| {
            let async_gen = handler.call1(py, (request,))?;

            loop {
                // Get next item from async generator
                let item = async_gen.call_method0(py, "__anext__")?;

                // If StopAsyncIteration, break
                // Otherwise, send item through channel
                tx.send(item).await?;
            }
        });
    });

    Ok(ReceiverStream::new(rx))
}
```

**For Client Streaming:**
```rust
// Request: Stream of messages from Rust to Python
// Response: Single message from Python to Rust

pub async fn execute_client_streaming_handler(
    &self,
    handler: PyObject,
    request_stream: impl Stream<Item = Vec<u8>>,
) -> Result<Vec<u8>, RpcError> {
    let (tx, rx) = mpsc::unbounded_channel();

    // Forward Rust stream to Python async generator
    tokio::spawn(async move {
        pin_mut!(request_stream);
        while let Some(item) = request_stream.next().await {
            tx.send(item).await?;
        }
    });

    // Execute handler with async iterator
    Python::with_gil(|py| {
        let py_stream = create_python_async_iterator(py, rx);
        let result = handler.call1(py, (py_stream,))?;

        // Await the async result
        let coroutine = result;
        self.execute_coroutine(py, coroutine).await
    })
}
```

### Integration with Persistent Event Loop

The existing `PythonEventLoopExecutor` needs extension:

```rust
// src/python/event_loop.rs

pub enum StreamingRequest {
    ServerStreaming {
        handler: PyObject,
        request: Vec<u8>,
        response_tx: mpsc::UnboundedSender<Vec<u8>>,
    },
    ClientStreaming {
        handler: PyObject,
        request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        response_tx: oneshot::Sender<Vec<u8>>,
    },
    Bidirectional {
        handler: PyObject,
        request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        response_tx: mpsc::UnboundedSender<Vec<u8>>,
    },
}
```

## Implementation Phases

### Phase 1: Server Streaming (1→N)
**Scope:** Single request → Multiple responses
**Complexity:** Medium
**Estimated Effort:** 2-3 days

**Tasks:**
1. Extend `PythonEventLoopExecutor` with server streaming support
2. Add `register_server_streaming()` to `PyRpcServer`
3. Bridge Python async generator to Rust `Stream`
4. Add tests for various yield patterns
5. Add benchmarks for streaming throughput

### Phase 2: Client Streaming (N→1)
**Scope:** Multiple requests → Single response
**Complexity:** Medium
**Estimated Effort:** 2-3 days

**Tasks:**
1. Create Python async iterator from Rust `Stream`
2. Add `register_client_streaming()` to `PyRpcServer`
3. Handle stream completion and errors
4. Add tests for various consumption patterns
5. Add benchmarks

### Phase 3: Bidirectional Streaming (N→M)
**Scope:** Multiple requests → Multiple responses
**Complexity:** High
**Estimated Effort:** 3-4 days

**Tasks:**
1. Combine server + client streaming patterns
2. Add `register_bidirectional()` to `PyRpcServer`
3. Handle concurrent sending and receiving
4. Add comprehensive tests
5. Add benchmarks for bidirectional throughput

### Phase 4: Client-Side Streaming Support
**Scope:** Python clients calling Rust streaming servers
**Complexity:** Medium
**Estimated Effort:** 2-3 days

**Tasks:**
1. Add `call_server_streaming()` to `PyRpcClient`
2. Add `call_client_streaming()` to `PyRpcClient`
3. Add `call_bidirectional()` to `PyRpcClient`
4. Python wrapper for `PyAsyncStream`
5. End-to-end integration tests

## Technical Challenges

### 1. Async Generator Iteration in Event Loop Thread

**Challenge:** Python async generators need to be iterated in the event loop thread.

**Solution:** Use `asyncio` methods like `__anext__()` and handle `StopAsyncIteration`:
```python
# In the event loop thread
async def iterate_generator(gen):
    while True:
        try:
            item = await gen.__anext__()
            yield item
        except StopAsyncIteration:
            break
```

### 2. GIL Management for Streaming

**Challenge:** Streaming involves multiple GIL acquisitions per stream.

**Solution:** Same pattern as unary - acquire GIL only when calling into Python:
```rust
loop {
    // Release GIL while waiting for next request
    let request = request_rx.recv().await;

    // Acquire GIL to call Python generator
    let response = Python::with_gil(|py| {
        generator.call_method0(py, "__anext__")
    });
}
```

### 3. Error Propagation

**Challenge:** Errors in async generators need to propagate correctly.

**Solution:** Wrap in `StreamError` and propagate through channels:
```rust
enum StreamItem {
    Data(Vec<u8>),
    Error(RpcError),
    End,
}
```

### 4. Backpressure

**Challenge:** Fast producers overwhelming slow consumers.

**Solution:** Use bounded channels with configurable buffer sizes:
```rust
let (tx, rx) = mpsc::channel(buffer_size);
```

## Performance Expectations

Based on current unary handler performance (~4,600 calls/sec):

- **Server Streaming:** ~4,000 messages/sec per stream
- **Client Streaming:** ~3,500 messages/sec per stream
- **Bidirectional:** ~3,000 messages/sec per stream

Degradation expected due to:
- Multiple GIL acquisitions per stream
- Channel overhead for forwarding
- Async generator iteration overhead

## Testing Strategy

### Unit Tests
- Python async generator → Rust Stream conversion
- Rust Stream → Python async iterator conversion
- Error handling and propagation
- Stream cancellation

### Integration Tests
- End-to-end server streaming
- End-to-end client streaming
- End-to-end bidirectional
- Multiple concurrent streams

### Performance Tests
- Throughput benchmarks (messages/sec)
- Latency benchmarks (ms/message)
- Memory usage under load
- Comparison with Rust streaming handlers

## Alternative Approaches Considered

### ❌ Approach 1: Callback-Based API
```python
def handler(request, send_response):
    for i in range(10):
        send_response(data)
```
**Rejected:** Not idiomatic Python, harder to use than async generators.

### ❌ Approach 2: Queue-Based API
```python
async def handler(request, response_queue):
    await response_queue.put(data)
```
**Rejected:** More complex than async generators, no clear advantage.

### ✅ Approach 3: Async Generator API (SELECTED)
```python
async def handler(request):
    yield data
```
**Selected:** Most Pythonic, leverages language features, familiar pattern.

## Documentation Requirements

1. **User Guide Section:** Python streaming handlers
2. **API Reference:** `register_server_streaming()`, etc.
3. **Examples:** One for each streaming type
4. **Migration Guide:** Upgrading from unary to streaming
5. **Performance Guide:** Best practices for streaming

## Success Criteria

- ✅ All three streaming types implemented
- ✅ Python client can consume Rust streaming servers
- ✅ Python server can serve streaming requests
- ✅ Performance ≥ 3,000 messages/sec per stream
- ✅ All tests passing
- ✅ Comprehensive documentation

## References

- Current unary implementation: `src/python/event_loop.rs`
- Rust streaming implementation: `src/streaming.rs`
- Python AsyncIO documentation: https://docs.python.org/3/library/asyncio-stream.html
- PyO3 async documentation: https://pyo3.rs/main/ecosystem/async-await

## Timeline Estimate

**Total:** 9-13 days for full implementation

- Phase 1 (Server Streaming): 2-3 days
- Phase 2 (Client Streaming): 2-3 days
- Phase 3 (Bidirectional): 3-4 days
- Phase 4 (Client-Side): 2-3 days

**Note:** This assumes the persistent event loop executor foundation is solid (✅ completed).

---

**Last Updated:** 2025-11-13
**Next Review:** When starting streaming implementation
