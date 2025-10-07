# Getting Started

This tutorial mirrors the `examples/basic_greeting` sample and shows, step by
step, how to install RpcNet, run the `rpcnet-gen` CLI, and integrate the
generated code into your own project.

## Step 0: Prerequisites

- Rust 1.75+ (`rustup show` to confirm)
- `cargo` on your `PATH`
- macOS or Linux (QUIC/TLS support is bundled through `s2n-quic`)

## Step 1: Create a new crate

```bash
cargo new hello-rpc
cd hello-rpc
```

## Step 2: Add the RpcNet runtime crate

```bash
cargo add rpcnet
```

RpcNet enables the high-performance `perf` feature by default. If you need to
opt out (e.g. another allocator is already selected), edit `Cargo.toml`:

```toml
[dependencies]
rpcnet = { version = "0.1", default-features = false }
```

You will also want `serde` for request/response types, just like the example:

```toml
serde = { version = "1", features = ["derive"] }
```

## Step 3: Install the rpcnet-gen CLI

Starting with v0.1.0, the CLI is included by default when you install rpcnet:

```bash
cargo install rpcnet  # CLI automatically included!
```

Verify the install:

```bash
rpcnet-gen --help
```

You should see the full usage banner:

```
Generate RPC client and server code from service definitions

Usage: rpcnet-gen [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>    Input .rpc file (Rust source with service trait)
  -o, --output <OUTPUT>  Output directory for generated code [default: src/generated]
      --server-only      Generate only server code
      --client-only      Generate only client code
      --types-only       Generate only type definitions
  -h, --help             Print help
  -V, --version          Print version
```

## Step 4: Author a service definition

Create `src/greeting.rpc.rs` describing your protocol. The syntax is ordinary
Rust with a `#[rpcnet::service]` attribute, so you can leverage the compiler and
IDE tooling while you design the API:

```rust
// src/greeting.rpc.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GreetingError {
    EmptyName,
    InvalidInput(String),
}

#[rpcnet::service]
pub trait Greeting {
    async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
}
```

## Step 5: Generate client and server code

Point the CLI at the `.rpc` file and choose an output directory. Here we mirror
`examples/basic_greeting` by writing into `src/generated`:

```bash
rpcnet-gen --input src/greeting.rpc.rs --output src/generated
```

The CLI confirms what it created:

```
📦 Generating code for service: Greeting
  ✅ Generated server: src/generated/greeting/server.rs
  ✅ Generated client: src/generated/greeting/client.rs
  ✅ Generated types: src/generated/greeting/types.rs

✨ Code generation complete!

📝 Add the following to your code to use the generated service:
    #[path = "generated/greeting/mod.rs"]
    mod greeting;
    use greeting::*;
```

Inspect the directory to see the modules that were created—this matches the
layout under `examples/basic_greeting/generated/`:

```
src/generated/
└── greeting/
    ├── client.rs   # async client wrapper for calling the service
    ├── mod.rs      # re-exports so `use greeting::*` pulls everything in
    ├── server.rs   # server harness plus `GreetingHandler` trait
    └── types.rs    # request/response/error structs cloned from the .rpc file
```

`client.rs` exposes `GreetingClient`, `server.rs` wires your implementation into
the transport via `GreetingServer`, and `types.rs` contains the shared data
structures.

## Step 6: Wire the generated code into your project

Reference the generated module and bring the types into scope. For example,
in `src/main.rs`:

```rust
#[path = "generated/greeting/mod.rs"]
mod greeting;

use greeting::client::GreetingClient;
use greeting::server::{GreetingHandler, GreetingServer};
use greeting::{GreetRequest, GreetResponse, GreetingError};
use rpcnet::RpcConfig;
```

From here there are two pieces to wire up:

1. **Server** – implement the generated `GreetingHandler` trait and launch the
   harness. This mirrors `examples/basic_greeting/server.rs`:

   ```rust
   struct MyGreetingService;

   #[async_trait::async_trait]
   impl GreetingHandler for MyGreetingService {
       async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError> {
           Ok(GreetResponse { message: format!("Hello, {}!", request.name) })
       }
   }

   #[tokio::main]
   async fn main() -> anyhow::Result<()> {
       let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8080")
           .with_key_path("certs/test_key.pem")
           .with_server_name("localhost");

       GreetingServer::new(MyGreetingService, config).serve().await?;
       Ok(())
   }
   ```

   `GreetingServer::serve` handles QUIC I/O, wiring your implementation to the
   generated protocol handlers.

   **Tuning worker threads (optional).** By default Tokio uses the number of
   available CPU cores. To override this for RpcNet services, set
   `RPCNET_SERVER_THREADS` and build your runtime manually:

   ```rust
   fn main() -> anyhow::Result<()> {
       let worker_threads = rpcnet::runtime::server_worker_threads();

       let runtime = tokio::runtime::Builder::new_multi_thread()
           .worker_threads(worker_threads)
           .enable_all()
           .build()?;

       runtime.block_on(async {
           // existing async server logic goes here
           Ok::<_, anyhow::Error>(())
       })?;

       Ok(())
   }
   ```

   Run the binary with a custom thread count:

   ```bash
   RPCNET_SERVER_THREADS=8 cargo run
   ```

   Adjust the command if your server lives in a different binary target (for
   example `cargo run --bin my-server`).

   If you keep using the `#[tokio::main]` macro, Tokio will also honour the
   upstream `TOKIO_WORKER_THREADS` environment variable.

2. **Client** – construct `GreetingClient` to invoke the RPC. Compare with
   `examples/basic_greeting/client.rs`:

   ```rust
   #[tokio::main]
   async fn main() -> anyhow::Result<()> {
       let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
           .with_server_name("localhost");

       let server_addr = "127.0.0.1:8080".parse()?;
       let client = GreetingClient::connect(server_addr, config).await?;

       let response = client.greet(GreetRequest { name: "World".into() }).await?;
       println!("Server replied: {}", response.message);
       Ok(())
   }
   ```

The generated client takes care of serialization, TLS, and backpressure while
presenting an async function per RPC method.

## Step 7: Build and run

Compile and execute as usual:

```bash
cargo build
cargo run
```

While you experiment, keep the reference example nearby:

```bash
ls examples/basic_greeting
# client.rs  generated/  greeting.rpc.rs  server.rs
```

Comparing your project with the example is a quick way to confirm the wiring
matches what the CLI expects.

## Where to go next

- Read the [rpcnet-gen CLI guide](rpcnet-gen.md) for advanced flags such as
  `--server-only`, `--client-only`, and custom output paths.
- Explore the [Concepts](concepts.md) chapter for runtime fundamentals,
  server/client wiring, and streaming patterns.
