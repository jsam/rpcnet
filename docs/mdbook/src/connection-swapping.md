# Connection Swapping for Low-Latency Serving

QUIC lets a server move a live connection between back-ends without forcing the
client to reconnect. RpcNet rides on top of QUIC so the same behaviour is easy
to demonstrate: a client connects to a small “director” process, the director
hands the stream to a worker, and when that worker fails the client reuses the
same QUIC connection to talk to the next healthy worker. The
`examples/connection_swap` crate wires the pieces together.

## Architecture Overview

```
                     +------------------------------+
                     |  Connection Director          |
                     |  (RpcNet server + router)     |
                     +------+-----------------------+
                            |
                            | streaming RPC
                            |
             +--------------+---------------+
             |                              |
        +----v-----+                   +----v-----+
        | Worker A |                   | Worker B |
        +----+-----+                   +----+-----+
             |    simulated failure         |
             +------------------------------+

Streaming Client ⇄ Director ⇄ Current Worker
```

* The **Streaming Client** holds a single QUIC session open to the director and
  repeatedly issues `call_server_streaming`. Frames prefixed with `[connected]`
  announce which worker owns the stream and include a `connection=` identifier;
  `[token]` frames carry the generated tokens with the same connection id.
* The **Director** keeps a small worker pool. Every request increments a stream
  ID so the logs can follow it from arrival, to assignment, to failure / retry.
* **Workers** deserialize a `WorkerAssignment`, log the hand-off, send a
  `[connected]` frame back to the client, emit tokens once per second, and after
  fifteen seconds simulate a failure to force a swap.

## Example Layout

```
examples/connection_swap/
├── Cargo.toml
├── src
│   ├── bin
│   │   ├── director.rs   # broker that chooses a worker
│   │   ├── client.rs     # streaming client binary
│   │   └── worker.rs     # worker process that emits tokens
│   ├── client_app.rs     # reusable client logic
│   ├── director.rs       # worker pool and routing logic
│   ├── protocol.rs       # worker registration types
│   └── worker.rs         # (library helper, not used in the binaries)
```

## Running the Example

Use four terminals so each component runs independently:

```bash
# Terminal 1 – worker A
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 2 – worker B
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 3 – director (the broker)
CONNECTION_SWAP_DIRECTOR_ADDR=127.0.0.1:61000 \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin director

# Terminal 4 – client (never reconnects)
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin client
```

### What the Logs Show

**Director**

```
INFO connection_swap::director] worker.addr=127.0.0.1:62001 worker.label=worker-a count=1 registered worker
INFO connection_swap::director] stream.id=1 connection.id=conn-1 prompt=prompt-... client opened stream
INFO connection_swap::director] stream.id=1 connection.id=conn-1 worker.addr=127.0.0.1:62001 worker.label=worker-a assigning stream to worker
WARN connection_swap::director] stream.id=1 connection.id=conn-1 worker.addr=127.0.0.1:62001 worker.label=worker-a worker stream ended with error
INFO connection_swap::director] stream.id=1 connection.id=conn-1 asking client to retry on next worker
INFO connection_swap::director] stream.id=2 connection.id=conn-2 worker.addr=127.0.0.1:62002 worker.label=worker-b assigning stream to worker
```

**Worker**

```
INFO worker] worker=worker-a stream.id=1 connection.id=conn-1 prompt=prompt-... accepted handoff from director
INFO worker] worker=worker-a stream.id=1 connection.id=conn-1 seq=1 sending token
WARN worker] worker=worker-a stream.id=1 connection.id=conn-1 simulating failure after 15s
```

**Client**

```
INFO connection_swap::client_app] connection.id=conn-1 worker=worker-a stream.id=1 worker assigned to stream
INFO connection_swap::client_app] connection.id=conn-1 worker=worker-a stream.id=1 sequence=1 received token
INFO connection_swap::client_app] stream ended; requesting another worker err=StreamError("worker-a simulated failure")
INFO connection_swap::client_app] connection.id=conn-1 worker=worker-b stream.id=1 worker assigned to stream
```

The three logs together make it easy to follow the connection as it bounces
between workers without the client reconnecting.

### Environment Variables

| Variable                          | Default           | Description                                     |
|-----------------------------------|-------------------|-------------------------------------------------|
| `CONNECTION_SWAP_DIRECTOR_ADDR`   | `127.0.0.1:61000` | Address the director binds to                   |
| `CONNECTION_SWAP_DIRECTOR_TARGET` | same as bind      | Address the client connects to (for NAT setups) |
| `CONNECTION_SWAP_WORKER_ADDR`     | —                 | Address for each worker process                 |
| `CONNECTION_SWAP_WORKER_LABEL`    | `worker`          | Label used in log output                        |

### Certificates

All binaries use the shared PEM files under `certs/`. Paths are canonicalised at
runtime so you can launch the binaries from anywhere inside the repository.

## Extending the Pattern

This demo keeps everything on one machine, but the same control flow works in a
real deployment: use `RpcServer::drive_connection` to pass the QUIC connection to
another process, or front the workers with a QUIC-aware proxy that rewrites
connection IDs. Swap in real health checks instead of simulated failures and
persist any state (chat history, KV caches, etc.) you need for seamless
handoffs.
