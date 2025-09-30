# Connection Swap Example

This example demonstrates how RpcNet can move a *live* QUIC connection between
backend workers without forcing the client to reconnect. A single client opens a
streaming RPC to a **director**. The director immediately hands the connection
over to an available **worker**. After a short period the worker simulates a
failure which causes the director to reclaim the same connection and hand it to
another worker. The client never re-establishes TLS— tokens continue to flow over
the original connection.

## Components

| Component | Binary | Responsibility |
|-----------|--------|----------------|
| Director  | `director` | Accepts client connections, keeps a worker pool, forwards streaming traffic, and reassigns connections when workers fail or recover. |
| Worker    | `worker` | Registers with the director, handles the streaming RPC (`inference.generate`), emits tokens, and periodically simulates a failure. |
| Client    | `client` | Connects to the director and issues a long-running streaming request. It logs every frame to show which worker currently serves the connection. |

All processes share the same TLS certificate/key pair in `certs/`.

## Expected Behaviour

1. Director starts listening on `127.0.0.1:61000` and waits for workers to
   register.
2. Worker A (port `62001`) and Worker B (port `62002`) register with the
   director. Each worker logs the registration result.
3. The client connects to the director and issues a streaming request. The
   director logs `client opened stream` together with a **connection id** of the
   form `conn-<n>`.
4. The director assigns the connection to the preferred worker. The log line
   includes the connection id and worker address.
5. The worker receives a `WorkerAssignment`, logs the stream id/connection id
   and sends a `[connected]` frame to the client. Subsequent tokens are logged as
   `[token] connection=<...> worker=<...> stream=<...> seq=<...>`.
6. After ~15 seconds the worker simulates a failure (and prints the failure to the stdout log). The worker, director, and
   client all log the event with the same `connection.id`, proving the hand-off.
7. The director immediately reassigns the connection to the other worker. A new
   `[connected]` frame arrives at the client but with the **same** connection id.
8. The sequence repeats as workers recover and re-fail, demonstrating repeated
   connection swaps without the client reconnecting.

## Running the Example

Use four terminals. Start the director **before** the workers so their
registration handshakes succeed quickly.

```bash
# Terminal 1 – director
CONNECTION_SWAP_DIRECTOR_ADDR=127.0.0.1:61000 \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin director

# Terminal 2 – worker A
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 3 – worker B
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 4 – client
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin client
```

### Successful Run Checklist

Look for the following lines (values will vary) to ensure the swap works end to
end.

- **Client**
  ```text
  INFO connection_swap::client_app] prompt=prompt-... issuing streaming request to director
  INFO connection_swap::client_app] stream opened, waiting for frames
  INFO connection_swap::client_app] connection.id=conn-1 worker=worker-a stream.id=1 worker assigned to stream
  INFO connection_swap::client_app] connection.id=conn-1 worker=worker-a stream.id=1 sequence=1 received token
  INFO connection_swap::client_app] stream ended; requesting another worker err=StreamError("worker-a simulated failure")
  INFO connection_swap::client_app] connection.id=conn-1 worker=worker-b stream.id=1 worker assigned to stream
  ```

- **Director**
  ```text
  INFO connection_swap::director] stream.id=1 connection.id=conn-1 prompt=prompt-... client opened stream
  INFO connection_swap::director] stream.id=1 connection.id=conn-1 worker.addr=127.0.0.1:62001 worker.label=worker-a assigning stream to worker
  WARN connection_swap::director] stream.id=1 connection.id=conn-1 worker.addr=127.0.0.1:62001 worker.label=worker-a worker stream ended with error
  INFO connection_swap::director] stream.id=1 connection.id=conn-1 asking client to retry on next worker
  INFO connection_swap::director] stream.id=1 connection.id=conn-1 worker.addr=127.0.0.1:62002 worker.label=worker-b assigning stream to worker
  ```

- **Worker**
  ```text
  INFO worker] worker=worker-a stream.id=1 connection.id=conn-1 prompt=prompt-... accepted handoff from director
  INFO worker] worker=worker-a stream.id=1 connection.id=conn-1 seq=1 sending token
  WARN worker] worker=worker-a stream.id=1 connection.id=conn-1 simulating failure after 15s
  ```

The shared `connection.id` confirms that the same QUIC connection is reused even
as the serving worker changes.

## Troubleshooting

- **Workers fail to register**: ensure the director is running first and that
  both worker processes use `CONNECTION_SWAP_DIRECTOR_TARGET` pointing at the
  director address.
- **No `[connected]` frames**: check the director log for `connecting to worker`
  and worker log for `stream handler invoked; waiting for assignment`. Missing
  lines indicate the handoff is stalled.
- **`Operation not permitted (os error 1)`**: a local security policy is blocking
  the QUIC socket bind. Run the commands outside of restricted environments or
  adjust permissions.

## Files of Interest

- `connection_swap.rpc.rs` – service definition consumed by `rpcnet-gen`.
- `src/generated/directorregistry/` – typed client/server modules produced by the generator.
- `src/director.rs` – director implementation and worker pool logic.
- `src/bin/worker.rs` – standalone worker binary that logs connection swaps.
- `src/client_app.rs` – reusable client helper with connection-aware logging.
- `src/protocol.rs` – connection hand-off payload shared between director and workers.
- `docs/mdbook/src/connection-swapping.md` – extended documentation including
  architecture diagrams and log examples.
