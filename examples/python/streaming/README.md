# RpcNet Streaming Example

Demonstrates RPC with **Rust server** and **Python client** using type-safe code generation.

**Current Status:** ✅ Unary RPC working | 🚧 Streaming patterns coming soon

## Quick Start

All commands should be run from `examples/python/streaming/` directory.

### 1. Generate Python Client Code

```bash
cd /Users/samuel.picek/inputlayer/rpcnet
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/python/streaming/streaming.rpc.rs \
  --output examples/python/streaming/streamingservice \
  --python
```

### 2. Build Rust Server

```bash
cargo build --release
```

### 3. Start Server

```bash
# Terminal 1: Start server (default: 127.0.0.1:50052)
target/release/server

# Or with custom address
BIND_ADDR=127.0.0.1:8080 target/release/server
```

### 4. Run Python Clients

Four client examples demonstrate each communication pattern:

```bash
# Terminal 2: Run clients

# 1. Unary RPC (✅ working)
/Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python unary_client.py

# 2. Server Streaming (🚧 coming soon)
/Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python server_streaming_client.py

# 3. Client Streaming (🚧 coming soon)
/Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python client_streaming_client.py

# 4. Bidirectional Streaming (🚧 coming soon)
/Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python bidi_streaming_client.py

# Or run the combined demo
/Users/samuel.picek/inputlayer/rpcnet/.venv/bin/python client.py
```

## Example Output

```
╔====================================================================╗
║                    RpcNet Python Client Demo                       ║
╚====================================================================╝

Server: 127.0.0.1:50052
Certificate: ../../../certs/test_cert.pem

🔌 Connecting to server...
✅ Connected!

======================================================================
📨 Testing Unary RPC (single request/response)
======================================================================

📤 Sending: Hello from Python client #1!
📥 Response: Server received: Hello from Python client #1!
   Timestamp: 1700000000

📤 Sending: Hello from Python client #2!
📥 Response: Server received: Hello from Python client #2!
   Timestamp: 1700000001

📤 Sending: Hello from Python client #3!
📥 Response: Server received: Hello from Python client #3!
   Timestamp: 1700000002

======================================================================
🎉 Demo completed successfully!
======================================================================
```

## Architecture

```
┌────────────────────────────────────┐
│      Rust Server (server.rs)       │
│                                    │
│  ✅ Unary RPC (working)            │
│  🚧 Server streaming (planned)     │
│  🚧 Client streaming (planned)     │
│  🚧 Bidirectional (planned)        │
│                                    │
│  Uses register_typed_polyglot      │
│  for type-safe MessagePack RPC     │
└────────────┬───────────────────────┘
             │
             │ QUIC/TLS + MessagePack
             │
┌────────────▼───────────────────────┐
│   Python Client (client.py)        │
│                                    │
│  StreamingServiceClient            │
│  - unary() ✅                      │
│  - server_stream() 🚧              │
│  - client_stream() 🚧              │
│  - bidi_stream() 🚧                │
│                                    │
│  Auto-generated from .rpc.rs       │
└────────────────────────────────────┘
```

## Files

- `streaming.rpc.rs` - Service definition with all RPC patterns
- `src/server.rs` - Rust server implementation
- **Python Clients:**
  - `unary_client.py` - Unary RPC demo (✅ working)
  - `server_streaming_client.py` - Server streaming demo (🚧 ready)
  - `client_streaming_client.py` - Client streaming demo (🚧 ready)
  - `bidi_streaming_client.py` - Bidirectional streaming demo (🚧 ready)
  - `client.py` - Combined demo
- `streamingservice/` - Auto-generated Python bindings (gitignored)

## Communication Patterns

### 1. Unary RPC (✅ Working)

**Single request → Single response**

The simplest pattern. Client sends one request, server returns one response.

```python
# unary_client.py
request = UnaryRequest(message="Hello!")
response = await client.unary(request)
print(response.reply)  # "Server received: Hello!"
```

**Use cases:** Simple queries, commands, CRUD operations

---

### 2. Server Streaming (🚧 Coming Soon)

**Single request → Stream of responses**

Client sends one request, server responds with a stream of messages.

```python
# server_streaming_client.py
request = ServerStreamRequest(count=10, prefix="Item")
response_stream = await client.server_stream(request)

async for response in response_stream:
    print(f"Received: {response.item}")
```

**Use cases:**
- Progress updates for long operations
- Real-time data feeds (stock prices, sensor data)
- Paginated results
- Event notifications

---

### 3. Client Streaming (🚧 Coming Soon)

**Stream of requests → Single response**

Client sends a stream of messages, server returns one aggregated response.

```python
# client_streaming_client.py
async def send_values():
    for value in [10, 20, 30, 40, 50]:
        yield ClientStreamRequest(value=value)

response = await client.client_stream(send_values())
print(f"Sum: {response.sum}, Count: {response.count}")
```

**Use cases:**
- File uploads (chunked transfer)
- Metrics/telemetry collection
- Batch data ingestion
- Log aggregation

---

### 4. Bidirectional Streaming (🚧 Coming Soon)

**Stream ↔ Stream**

Both client and server send streams simultaneously. Most flexible pattern.

```python
# bidi_streaming_client.py
async def send_messages():
    for text in ["Hello", "World", "RpcNet"]:
        yield BidiStreamRequest(text=text)

response_stream = await client.bidi_stream(send_messages())

async for response in response_stream:
    print(f"Echo: {response.echo}, Reversed: {response.reversed}")
```

**Use cases:**
- Chat applications
- Real-time collaboration
- Game state synchronization
- Live data transformation pipelines

---

## Implementation

### Server (Rust)

```rust
// Register unary handler
let unary_handler = move |request: UnaryRequest| async move {
    Ok::<UnaryResponse, RpcError>(UnaryResponse {
        reply: format!("Server received: {}", request.message),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    })
};
server.register_typed_polyglot("StreamingService.unary", unary_handler).await;
```

### Client (Python)

```python
client = await StreamingServiceClient.connect(
    addr="127.0.0.1:50052",
    cert_path="../../../certs/test_cert.pem",
    server_name="localhost",
)

request = UnaryRequest(message="Hello!")
response = await client.unary(request)
print(response.reply)  # "Server received: Hello!"
```

## Configuration

**Server Environment Variables:**
- `BIND_ADDR` - Server bind address (default: `127.0.0.1:50052`)
- `RUST_LOG` - Logging level (e.g., `RUST_LOG=info`)

**Client Environment Variables:**
- `SERVER_ADDR` - Server address (default: `127.0.0.1:50052`)
- `CERT_PATH` - TLS certificate path (default: `../../../certs/test_cert.pem`)

## Troubleshooting

**"Cannot find test_cert.pem"**
```bash
# From repo root
./generate_certs.sh
```

**"Connection refused"**
- Ensure server is running: `target/release/server`
- Check address matches between client and server

**"Module not found: streamingservice"**
```bash
# Regenerate Python bindings
cd /Users/samuel.picek/inputlayer/rpcnet
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/python/streaming/streaming.rpc.rs \
  --output examples/python/streaming/streamingservice \
  --python
```

**Build fails with "no method named register_typed_polyglot_*_stream"**
- This is expected - streaming methods are not yet implemented in the core library
- Only unary RPC works currently

## Future: Streaming Patterns

When streaming support is added to the core library, this example will demonstrate:

- **Server Streaming**: Single request → Stream of responses
- **Client Streaming**: Stream of requests → Single response  
- **Bidirectional**: Stream ↔ Stream

The service definitions in `streaming.rpc.rs` are ready for when streaming is implemented.

## Type Safety

All types are defined in `streaming.rpc.rs` and code-generated:

- ✅ Compile-time type checking in Rust
- ✅ Runtime type validation in Python  
- ✅ IDE autocomplete
- ✅ Automatic MessagePack serialization
