# Streaming Overview

RpcNet builds streaming on top of QUIC bidirectional streams, letting clients
and servers exchange sequences of frames concurrently. This chapter explains the
core terminology, how the helpers map to underlying QUIC behaviour, and which
features to reach for when designing real-time APIs.

## What “streaming” means in RpcNet

Each streaming RPC opens a fresh QUIC bidirectional stream:

- Frames are transported as length-prefixed `Vec<u8>` payloads.
- Upload and download directions operate independently; the client can keep
  sending while the server responds, and vice versa.
- Either side sends a zero-length frame to signal end-of-stream.

RpcNet exposes three convenience helpers that mirror gRPC-style semantics:

| Pattern                 | Helper on `RpcClient`           | Typical use case                         |
|-------------------------|---------------------------------|------------------------------------------|
| Bidirectional streaming | `call_streaming`                | Chat, collaborative editing, turn-taking |
| Server streaming        | `call_server_streaming`         | Live dashboards, subscriptions, long poll|
| Client streaming        | `call_client_streaming`         | Batched uploads, telemetry aggregation   |

The server registers a single handler API (`register_streaming`) for all three
patterns; the difference lies in how the client constructs the request stream
and how many responses it expects.

## Frame format

RpcNet’s streaming frames follow this layout:

```
<u32 payload_length in little endian><payload bytes>
```

- `payload_length == 0` means “no more frames”.
- Payloads contain arbitrary user-defined bytes; most examples serialize using
  `bincode` or `serde_json`.
- The library allocates buffers lazily and only keeps a single frame in memory
  per direction.

## Bidirectional streaming in detail

Use `RpcClient::call_streaming` when both sides continuously trade messages:

```rust
let responses = client.call_streaming("chat", outbound_frames).await?;
```

The client passes an async `Stream<Item = Vec<u8>>` and receives another stream
for responses. RpcNet multiplexes both directions on a single QUIC stream. The
server handler receives an async stream of request frames and must return an
async stream of `Result<Vec<u8>, RpcError>` responses.

Choose this mode when:

- Each request needs a corresponding response (command/reply flow).
- Both parties produce data over time (whiteboard sessions, multiplayer games).
- You want to push updates without closing the upload direction.

## Server streaming

`RpcClient::call_server_streaming` wraps `call_streaming` for the common case
where the client sends **one** request and the server streams many responses:

```rust
let stream = client.call_server_streaming("subscribe", request_bytes).await?;
```

On the server, the handler still observes a request stream; most implementations
read the first frame as the subscription and ignore additional frames. Use this
pattern when the server drives the timeline (market data, notifications,
progress updates).

## Client streaming

`RpcClient::call_client_streaming` handles the inverse: the client uploads many
frames and waits for a single aggregated response.

```rust
let response = client.call_client_streaming("upload", outbound_frames).await?;
```

The server consumes every inbound frame before yielding exactly one response
frame. This pattern pairs well with compression or summarisation (log shipping,
bulk metrics, video chunk ingestion).

## Keep-alive and flow control

- `RpcConfig::with_keep_alive_interval` controls heartbeat frames at the QUIC
  layer, keeping otherwise idle streams alive.
- Flow control is managed by s2n-quic; RpcNet reads and writes asynchronously,
  so slow consumers only backpressure their own stream, not the entire
  connection.
- Because each RPC lives on a separate QUIC stream, you can run many streaming
  calls in parallel without head-of-line blocking.

## Error handling semantics

- Returning `Err(RpcError)` from a server response stream sends a generic error
  frame to the client and terminates the stream. Encode domain-specific errors
  inside your payloads when you need richer context.
- If the client drops its output stream early, the server handler eventually

  sees `None` from the inbound iterator and can clean up resources.
- Timeouts follow the same `DEFAULT_TIMEOUT` as unary calls, so linger only as
  long as your app requires.

## Choosing between streaming helpers

Ask yourself:

1. Does the client expect multiple responses? → Use server streaming.
2. Does the server expect multiple requests? → Use client streaming.
3. Do both sides talk repeatedly? → Use bidirectional streaming.

When none of the above apply, stick with unary RPCs—they offer simpler error
handling and deterministic retry behaviour.

## What’s next

- Jump to the [Streaming Walkthrough](streaming-example.md) for a complete
  telemetry example that covers every helper.
- Revisit [Concepts](concepts.md#streaming-patterns) if you need low-level API
  reminders or code snippets.

Armed with the terminology and behaviour described here, you can design
streaming endpoints with confidence and implement them using the detailed guide
in the next chapter.
