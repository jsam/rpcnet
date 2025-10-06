# Streaming Walkthrough

This end-to-end example builds a telemetry service that exercises every
streaming mode RpcNet offers: bidirectional chat, server streaming updates, and
client streaming uploads. Follow along to scaffold the project, implement the
handlers, and drive the flows from a client binary.

## Step 0: Prerequisites

- Rust 1.75+ (`rustup show` to confirm)
- `cargo` on your `PATH`
- macOS or Linux (TLS support is bundled via `s2n-quic`)

## Step 1: Create the project layout

```bash
cargo new telemetry-streams --bin
cd telemetry-streams
mkdir -p certs src/bin
rm src/main.rs  # we'll rely on explicit binaries instead of the default main
```

The example uses two binaries: `src/bin/server.rs` and `src/bin/client.rs`.

## Step 2: Declare dependencies

Edit `Cargo.toml` to pull in RpcNet and helper crates:

```toml
[package]
name = "telemetry-streams"
version = "0.1.0"
edition = "2021"

[dependencies]
rpcnet = "0.2"
serde = { version = "1", features = ["derive"] }
bincode = "1.3"
async-stream = "0.3"
futures = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

- `rpcnet` provides the client/server runtime.
- `async-stream` and `futures` help produce response streams on the server.
- `serde`/`bincode` handle payload serialization.
- Tokio is required because RpcNet is async-first.

## Step 3: Generate development certificates

RpcNet requires TLS material for QUIC. Create a self-signed pair for local
experiments:

```bash
openssl req -x509 -newkey rsa:4096 \
  -keyout certs/server-key.pem \
  -out certs/server-cert.pem \
  -days 365 -nodes \
  -subj "/CN=localhost"
```

The client reuses the public certificate file to trust the server.

## Step 4: Define shared data types

Expose a library module that both binaries can import. Create `src/lib.rs`:

```rust
// src/lib.rs
pub mod telemetry;
```

Now add the telemetry definitions in `src/telemetry.rs`:

```rust
// src/telemetry.rs
use rpcnet::RpcError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricReading {
    pub sensor: String,
    pub value: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LiveUpdate {
    pub sensor: String,
    pub rolling_avg: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub from: String,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ack {
    pub accepted: usize,
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, RpcError> {
    Ok(bincode::serialize(value)?)
}

pub fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, RpcError> {
    Ok(bincode::deserialize(bytes)?)
}
```

These helpers convert structures to and from the `Vec<u8>` payloads that
RpcNet transports.

## Step 5: Implement the streaming server

Create `src/bin/server.rs` with three handlers—one per streaming pattern:

```rust
// src/bin/server.rs
use async_stream::stream;
use futures::StreamExt;
use rpcnet::{RpcConfig, RpcServer};
use telemetry_streams::telemetry::{self, Ack, ChatMessage, LiveUpdate, MetricReading};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("certs/server-cert.pem", "127.0.0.1:9000")
        .with_key_path("certs/server-key.pem")
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);

    // Bidirectional chat: echo each message with a server tag.
    server
        .register_streaming("chat", |mut inbound| async move {
            stream! {
                while let Some(frame) = inbound.next().await {
                    let msg: ChatMessage = telemetry::decode(&frame)?;
                    let reply = ChatMessage {
                        from: "server".into(),
                        body: format!("ack: {}", msg.body),
                    };
                    yield telemetry::encode(&reply);
                }
            }
        })
        .await;

    // Server streaming: emit rolling averages for a requested sensor.
    server
        .register_streaming("subscribe_metrics", |mut inbound| async move {
            stream! {
                if let Some(frame) = inbound.next().await {
                    let req: MetricReading = telemetry::decode(&frame)?;
                    let mut window = vec![req.value];
                    for step in 1..=5 {
                        sleep(Duration::from_millis(500)).await;
                        window.push(req.value + step as f64);
                        let avg = window.iter().copied().sum::<f64>() / window.len() as f64;
                        let update = LiveUpdate { sensor: req.sensor.clone(), rolling_avg: avg };
                        yield telemetry::encode(&update);
                    }
                }
            }
        })
        .await;

    // Client streaming: collect readings and acknowledge how many we processed.
    server
        .register_streaming("upload_batch", |mut inbound| async move {
            stream! {
                let mut readings: Vec<MetricReading> = Vec::new();
                while let Some(frame) = inbound.next().await {
                    let reading: MetricReading = telemetry::decode(&frame)?;
                    readings.push(reading);
                }
                let ack = Ack { accepted: readings.len() };
                yield telemetry::encode(&ack);
            }
        })
        .await;

    let quic_server = server.bind()?;
    println!("Telemetry server listening on 127.0.0.1:9000");
    server.start(quic_server).await?;
    Ok(())
}
```

Key points:

- `register_streaming` receives a stream of request frames (`Vec<u8>`) and must
  return a stream of `Result<Vec<u8>, RpcError>` responses.
- The bidirectional handler echoes every inbound payload.
- The server-streaming handler reads a single subscription request and then
  pushes periodic updates without further client input.
- The client-streaming handler drains all incoming frames before returning one
  acknowledgement.

## Step 6: Implement the client

Create `src/bin/client.rs` to exercise each streaming helper:

```rust
// src/bin/client.rs
use futures::{stream, StreamExt};
use rpcnet::{RpcClient, RpcConfig, RpcError};
use telemetry_streams::telemetry::{self, Ack, ChatMessage, LiveUpdate, MetricReading};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("certs/server-cert.pem", "127.0.0.1:0")
        .with_server_name("localhost");

    let client = RpcClient::connect("127.0.0.1:9000".parse()?, config).await?;

    chat_demo(&client).await?;
    server_stream_demo(&client).await?;
    client_stream_demo(&client).await?;

    Ok(())
}

async fn chat_demo(client: &RpcClient) -> Result<(), RpcError> {
    println!("\n--- Bidirectional chat ---");
    let messages = vec![
        ChatMessage { from: "operator".into(), body: "ping".into() },
        ChatMessage { from: "operator".into(), body: "status?".into() },
    ];
    let outbound_frames: Vec<Vec<u8>> = messages
        .into_iter()
        .map(|msg| telemetry::encode(&msg).expect("serialize chat message"))
        .collect();
    let outbound = stream::iter(outbound_frames);
    let mut inbound = client.call_streaming("chat", outbound).await?;
    while let Some(frame) = inbound.next().await {
        let bytes = frame?;
        let reply: ChatMessage = telemetry::decode(&bytes)?;
        println!("reply: {}", reply.body);
    }
    Ok(())
}

async fn server_stream_demo(client: &RpcClient) -> Result<(), RpcError> {
    println!("\n--- Server streaming ---");
    let request = telemetry::encode(&MetricReading { sensor: "temp".into(), value: 21.0 })?;
    let mut updates = client
        .call_server_streaming("subscribe_metrics", request)
        .await?;
    while let Some(frame) = updates.next().await {
        let bytes = frame?;
        let update: LiveUpdate = telemetry::decode(&bytes)?;
        println!("rolling avg: {:.2}", update.rolling_avg);
    }
    Ok(())
}

async fn client_stream_demo(client: &RpcClient) -> Result<(), RpcError> {
    println!("\n--- Client streaming ---");
    let readings: Vec<Vec<u8>> = vec![
        MetricReading { sensor: "temp".into(), value: 21.0 },
        MetricReading { sensor: "temp".into(), value: 21.5 },
        MetricReading { sensor: "temp".into(), value: 22.0 },
    ]
    .into_iter()
    .map(|reading| telemetry::encode(&reading).expect("serialize reading"))
    .collect();
    let outbound = stream::iter(readings);
    let ack_frame = client
        .call_client_streaming("upload_batch", outbound)
        .await?;
    let ack: Ack = telemetry::decode(&ack_frame)?;
    println!("server accepted {} readings", ack.accepted);
    Ok(())
}
```

The client demonstrates:

- `call_streaming` for true bidirectional messaging.
- `call_server_streaming` when only the server produces a stream of frames.
- `call_client_streaming` to upload many frames and receive one response.

## Step 7: Run the scenario

Terminal 1 – start the server:

```bash
cargo run --bin server
```

Terminal 2 – launch the client:

```bash
cargo run --bin client
```

Expected output (trimmed for brevity):

```
--- Bidirectional chat ---
reply: ack: ping
reply: ack: status?

--- Server streaming ---
rolling avg: 21.00
rolling avg: 21.50
...

--- Client streaming ---
server accepted 3 readings
```

## Where to go next

- Revisit the [Concepts](concepts.md#streaming-patterns) chapter for API
  reference material.
- Combine streaming RPCs with code-generated unary services from the
  [Getting Started](getting-started.md) tutorial.
- Layer authentication, backpressure, or persistence around these handlers to
  match your production needs.
