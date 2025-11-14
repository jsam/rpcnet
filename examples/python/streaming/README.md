# Python Streaming RPC Examples

This directory contains examples demonstrating all three streaming patterns supported by RpcNet's Python bindings:

1. **Server Streaming (1→N)** - One request, multiple responses
2. **Client Streaming (N→1)** - Multiple requests, one response
3. **Bidirectional Streaming (N→M)** - Multiple requests, multiple responses

## Quick Start

**Easy way:** Use the provided runner script:

```bash
./examples/python/streaming/run_examples.sh
```

The script will:
- Check and build Python bindings if needed
- Generate TLS certificates if missing
- Present an interactive menu to run examples
- Option to launch all examples in separate terminals

**Manual way:** See prerequisites below.

## Prerequisites

1. Build the Python bindings:
   ```bash
   cd /path/to/rpcnet
   maturin develop --features python
   ```

2. Ensure TLS certificates exist:
   ```bash
   mkdir -p certs
   cd certs
   openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem -days 365 -nodes -subj "/CN=localhost"
   cd ..
   ```

## Testing the Examples

Each example has a corresponding test client to demonstrate the streaming functionality:

**Terminal 1: Start the server**
```bash
.venv/bin/python -u examples/python/streaming/server_streaming_example.py
```

**Terminal 2: Run the test client**
```bash
.venv/bin/python -u examples/python/streaming/test_server_streaming.py
```

Or use the interactive runner script which handles everything:
```bash
./examples/python/streaming/run_examples.sh
```

## Examples

### 1. Server Streaming (`server_streaming_example.py`)

**Pattern:** Client sends one request, server yields multiple responses

**Use Cases:**
- Streaming log entries
- Sending multiple search results
- Real-time data feeds
- Progress updates

**Run the server:**
```bash
python3 examples/python/streaming/server_streaming_example.py
```

**Test with a Rust client:**
```rust
// Send request: {"count": 5}
// Receive 5 responses, each with index, value, timestamp
```

The server will:
- Accept a request with `{"count": N}`
- Yield N responses, each containing an index and computed value
- Simulate processing delay between yields

### 2. Client Streaming (`client_streaming_example.py`)

**Pattern:** Client sends multiple requests, server returns one response

**Use Cases:**
- File uploads (receiving chunks)
- Data ingestion and batch processing
- Aggregating metrics or statistics
- Collecting sensor data

**Run the server:**
```bash
python3 examples/python/streaming/client_streaming_example.py
```

**Test with a Rust client:**
```rust
// Send multiple requests: {"data": [bytes...]}
// Receive one response with aggregated statistics
```

The server will:
- Receive multiple chunks from the client
- Aggregate all data
- Return a single response with statistics (chunk count, total bytes, hash)

### 3. Bidirectional Streaming (`bidirectional_streaming_example.py`)

**Pattern:** Client sends multiple requests, server yields multiple responses

**Use Cases:**
- Real-time chat
- Live data transformation/filtering
- Interactive processing pipelines
- Streaming analytics with immediate feedback

**Run the server:**
```bash
python3 examples/python/streaming/bidirectional_streaming_example.py
```

**Test with a Rust client:**
```rust
// Send multiple requests: {"message": "text"}
// Receive response for each message immediately
```

The server will:
- Receive messages from the client stream
- Transform each message (uppercase + prefix)
- Yield transformed responses immediately
- Process messages concurrently as they arrive

## Test Clients

Each server example has a corresponding test client:

### `test_server_streaming.py`
Tests the server streaming example by requesting 5 numbers and consuming the stream.

**Usage:**
```bash
# Start server first (Terminal 1)
.venv/bin/python -u examples/python/streaming/server_streaming_example.py

# Run test client (Terminal 2)
.venv/bin/python -u examples/python/streaming/test_server_streaming.py
```

### `test_client_streaming.py`
Tests the client streaming example by sending 5 text chunks to the server.

**Usage:**
```bash
# Start server first (Terminal 1)
.venv/bin/python -u examples/python/streaming/client_streaming_example.py

# Run test client (Terminal 2)
.venv/bin/python -u examples/python/streaming/test_client_streaming.py
```

### `test_bidirectional_streaming.py`
Tests the bidirectional streaming example by sending 5 messages and receiving transformed responses.

**Usage:**
```bash
# Start server first (Terminal 1)
.venv/bin/python -u examples/python/streaming/bidirectional_streaming_example.py

# Run test client (Terminal 2)
.venv/bin/python -u examples/python/streaming/test_bidirectional_streaming.py
```

## Implementation Details

### Server Streaming Handler

```python
async def stream_numbers(request_bytes: bytes):
    """Yields multiple responses for one request"""
    request = _rpcnet.msgpack_to_python_py(request_bytes)
    count = request.get("count", 10)

    for i in range(count):
        await asyncio.sleep(0.1)
        response = {"index": i, "value": i * i}
        yield _rpcnet.python_to_msgpack_py(response)

# Register with server
await server.register_server_streaming("stream_numbers", stream_numbers)
```

### Client Streaming Handler

```python
async def upload_file(request_stream):
    """Consumes multiple requests, returns one response"""
    total_bytes = 0

    async for chunk_bytes in request_stream:
        chunk = _rpcnet.msgpack_to_python_py(chunk_bytes)
        total_bytes += len(chunk["data"])

    response = {"total_bytes": total_bytes}
    return _rpcnet.python_to_msgpack_py(response)

# Register with server
await server.register_client_streaming("upload_file", upload_file)
```

### Bidirectional Streaming Handler

```python
async def echo_transform(request_stream):
    """Consumes and yields multiple messages"""
    async for request_bytes in request_stream:
        request = _rpcnet.msgpack_to_python_py(request_bytes)
        message = request.get("message", "")

        # Transform and yield immediately
        transformed = f"ECHO: {message.upper()}"
        response = {"transformed": transformed}
        yield _rpcnet.python_to_msgpack_py(response)

# Register with server (note: method is called register_bidirectional on server)
await server.register_bidirectional("echo_transform", echo_transform)
```

## Architecture

These examples use the **persistent event loop thread architecture** implemented in `src/python/event_loop.rs`:

- A dedicated OS thread maintains a persistent Python `asyncio` event loop
- Handlers execute in this event loop, with proper GIL management
- Performance: ~4,600 calls/sec with sub-millisecond latency
- The GIL is released while waiting for requests, allowing concurrent Python access

## Testing

All three patterns are covered by unit tests in:
- `tests/test_python_streaming.rs` - Handler structure tests
- Examples in this directory serve as integration tests

Run unit tests:
```bash
cargo test --test test_python_streaming --features python
```

## Notes

- All servers use port 900X (9001, 9002, 9003) to avoid conflicts
- Servers run indefinitely until Ctrl+C
- Each example includes detailed logging of incoming/outgoing messages
- MessagePack serialization is used for all data exchange
- TLS/QUIC transport provides secure, multiplexed connections

## Documentation

For more details on the Python streaming implementation, see:
- `PYTHON_ASYNC_LIMITATION.md` - Historical context and implementation details
- `docs/PYTHON_STREAMING_DESIGN.md` - Design document and architecture
