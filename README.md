<div align="center">

# RpcNet

[![PR Checks](https://github.com/jsam/rpcnet/workflows/PR%20Checks/badge.svg)](https://github.com/jsam/rpcnet/actions/workflows/pr-checks.yml)
[![Coverage](https://codecov.io/gh/jsam/rpcnet/branch/main/graph/badge.svg)](https://codecov.io/gh/jsam/rpcnet)
[![Crates.io](https://img.shields.io/crates/v/rpcnet.svg)](https://crates.io/crates/rpcnet)
[![Documentation](https://docs.rs/rpcnet/badge.svg)](https://docs.rs/rpcnet)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/jsam/rpcnet#license)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Downloads](https://img.shields.io/crates/d/rpcnet.svg)](https://crates.io/crates/rpcnet)
[![Stars](https://img.shields.io/github/stars/jsam/rpcnet?style=social)](https://github.com/jsam/rpcnet)

**A low latency RPC library for cluster building with full QUIC+TLS and SWIM support**

[Getting Started](#quick-start) • [Documentation](https://docs.rs/rpcnet) • [User Guide](https://jsam.github.io/rpcnet/) • [Examples](#examples) • [Benchmarks](#benchmarking)

</div>

---

## Features

### Core Features
- **🔒 TLS Security**: Built-in TLS 1.3 encryption and authentication
- **⚡ Async/Await**: Full async support with optimized Tokio runtime
- **📦 Binary Serialization**: Efficient data serialization with bincode
- **🛡️ Type Safety**: Strongly typed RPC calls with compile-time guarantees
- **🔧 Code Generation**: Generate type-safe client and server code from service definitions
- **⏱️ Timeout Handling**: Configurable request timeouts with automatic cleanup
- **🔍 Error Handling**: Comprehensive error types for robust applications
- **📊 Production Ready**: Battle-tested with extensive test coverage

### Cluster & Distributed Systems
- **🌐 Cluster Management**: Built-in distributed cluster support with automatic node discovery
- **🔄 Load Balancing**: Multiple strategies (Round Robin, Random, Least Connections)
- **💓 Health Checking**: Phi Accrual failure detection for accurate health monitoring
- **🗣️ Gossip Protocol**: SWIM-based gossip for efficient cluster communication
- **🏷️ Tag-Based Routing**: Route requests to workers by tags (role, zone, GPU/CPU, etc.)
- **📡 Event System**: Real-time cluster events (NodeJoined, NodeLeft, NodeFailed)
- **🔌 Connection Pooling**: Efficient connection reuse with configurable pool settings
- **🔁 Auto-Discovery**: Workers automatically discovered via gossip protocol
- **🛡️ Partition Detection**: Automatic detection and handling of network partitions

## 🚀 Performance

- Exceptional throughput with full encryption (exceeds HTTP/1.1 performance!)
- Modern transport with connection multiplexing and 0-RTT resumption
- Custom QUIC limits, efficient buffer management, and minimal allocations
- Handle 10k+ simultaneous streams per connection
- Under 100µs RTT over TLS overhead

## Installation

### Add RpcNet to Your Project

Add `rpcnet` to your `Cargo.toml`:

```toml
[dependencies]
rpcnet = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
async-trait = "0.1"

# Optional: For code generation in build.rs
[build-dependencies]
rpcnet = { version = "0.1.0", features = ["codegen"] }
```

### Installing the CLI Tool

The `rpcnet-gen` CLI tool generates type-safe client and server code from service definitions. **The CLI is included by default when you install rpcnet.**

```bash
# Install from crates.io (includes rpcnet-gen CLI)
cargo install rpcnet

# Or install from source
cargo install --path .
```

Verify installation:
```bash
rpcnet-gen --help
```

## Code Generation

RpcNet includes a code generator that creates type-safe client and server code from service definitions.

### 1. Create a Service Definition

Create a `.rpc.rs` file using standard Rust syntax:

```rust
// calculator.rpc.rs
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddRequest {
    pub a: i64,
    pub b: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddResponse {
    pub result: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CalculatorError {
    Overflow,
    InvalidInput(String),
}

#[rpcnet::service]
pub trait Calculator {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, CalculatorError>;
}
```

### 2. Generate Code

Use the CLI tool to generate client and server code:

```bash
# Generate code from service definition
rpcnet-gen --input calculator.rpc.rs --output src/generated

# Multiple files
rpcnet-gen --input definitions/ --output src/generated
```

### 3. Use Generated Code

#### Server Implementation

```rust
use rpcnet::RpcConfig;
use generated::calculator::{Calculator, CalculatorHandler, CalculatorServer};
use generated::calculator::{AddRequest, AddResponse, CalculatorError};

struct MyCalculator;

#[async_trait::async_trait]
impl CalculatorHandler for MyCalculator {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, CalculatorError> {
        let result = request.a.checked_add(request.b)
            .ok_or(CalculatorError::Overflow)?;
        Ok(AddResponse { result })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:8090");
    let server = CalculatorServer::new(MyCalculator, config);
    server.serve().await?;
    Ok(())
}
```

#### Client Usage

```rust
use rpcnet::RpcConfig;
use generated::calculator::{CalculatorClient, AddRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
        .with_server_name("localhost");
    
    let client = CalculatorClient::connect("127.0.0.1:8090".parse()?, config).await?;
    
    let response = client.add(AddRequest { a: 10, b: 20 }).await?;
    println!("Result: {}", response.result);
    
    Ok(())
}
```

### 4. Build Integration

Add to your `build.rs`:

```rust
fn main() {
    // Regenerate code when service definitions change
    println!("cargo:rerun-if-changed=definitions/");
    
    rpcnet::codegen::Builder::new()
        .input("definitions/calculator.rpc.rs")
        .output("src/generated")
        .build()
        .expect("Failed to generate RPC code");
}
```

## Quick Start

### Prerequisites

1. Add to your `Cargo.toml`:
```toml
[dependencies]
rpcnet = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

2. Generate TLS certificates (for development):
```bash
# Create a certs directory
mkdir certs && cd certs

# Generate a self-signed certificate for development
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem -days 365 -nodes \
  -subj "/CN=localhost"

cd ..
```

### Server Example

```rust
use rpcnet::{RpcServer, RpcConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8080")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);
    
    server.register("greet", |params| async move {
        let name = String::from_utf8_lossy(&params);
        let response = format!("Hello, {}!", name);
        Ok(response.into_bytes())
    }).await;

    let quic_server = server.bind()?;
    server.start(quic_server).await?;
    Ok(())
}
```

### Client Example

```rust
use rpcnet::{RpcClient, RpcConfig};

#[tokio::main]  
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_server_name("localhost");

    let client = RpcClient::connect("127.0.0.1:8080".parse().unwrap(), config).await?;
    let response = client.call("greet", b"World".to_vec()).await?;
    
    println!("Response: {}", String::from_utf8_lossy(&response));
    Ok(())
}
```

## Testing

RpcNet maintains **65%+ test coverage** with comprehensive unit tests, integration tests, and examples:

```bash
# Run all tests
cargo test

# Generate coverage report
make coverage

# Check coverage meets 65% threshold
make coverage-check

# Analyze coverage gaps
make coverage-gaps

# Test examples
cargo run --example basic_client_server
```

### Coverage Requirements

- **Overall Project**: 65% minimum coverage

See [docs/COVERAGE.md](docs/COVERAGE.md) for detailed coverage information and [TESTING.md](TESTING.md) for testing guidelines.

## Benchmarking

RpcNet includes comprehensive benchmarks demonstrating its exceptional performance:

```bash
# Run benchmarks (standard)
cargo bench

# Run benchmarks with performance optimizations (jemalloc allocator)
cargo bench --features perf

# Specific benchmark scenarios
cargo bench --bench simple max_throughput  # Test maximum throughput
cargo bench --bench simple concurrent      # Test concurrent operations
```

**Performance Highlights:**
- **172,000+ requests/second** with full QUIC+TLS encryption
- **Sub-millisecond latency** (< 0.1ms overhead) for local connections
- **10,000+ concurrent streams** per connection
- **Optimized memory usage** with BytesMut buffers and optional jemalloc allocator

## Examples

### Basic Examples (No Setup Required)

Simple examples using the low-level API that work immediately:

```bash
# Basic server and client
cargo run --example basic_server
cargo run --example basic_client

# Echo server with text/binary handling  
cargo run --example simple_echo_server
cargo run --example simple_echo_client
```

### Advanced Examples (Code Generation)

Complete, self-contained examples demonstrating code generation:

| Example | Description |
|---------|-------------|
| **`basic_greeting/`** | Simple request/response service |
| **`echo/`** | Binary data and multiple methods |
| **`file_transfer/`** | Chunked operations and stateful services |
| **`calculator/`** | Mathematical operations with error handling |
| **`concurrent_demo/`** | Concurrent operations and shared state |

```bash
# Generated code examples (require codegen feature)
cargo run --example basic_greeting_server --features codegen
cargo run --example basic_greeting_client --features codegen
```

### Cluster Examples

Production-ready cluster example demonstrating distributed systems:

```bash
# Terminal 1 - Start director/coordinator
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2 - Start worker A
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 3 - Start worker B
WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 4 - Start client
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client

# See examples/cluster/README.md for detailed setup and configuration
```

**Key cluster features demonstrated:**
- Automatic worker discovery via gossip protocol
- Load balancing strategies (Round Robin, Random, Least Connections)
- Phi Accrual failure detection with simulated failures
- Tag-based routing and filtering
- Connection pooling and reuse
- Zero-downtime worker failover and recovery

**📚 For comprehensive tutorials and documentation, see `cargo doc --open`**

## Documentation

### 📖 [Read the User Guide](https://jsam.github.io/rpcnet/)

The comprehensive user guide is available online at **[jsam.github.io/rpcnet](https://jsam.github.io/rpcnet/)** and includes:

- **📚 Complete Tutorial**: Step-by-step guide from basics to advanced patterns
- **🔨 Code Generation**: Complete guide to the code generation feature
- **🌐 Cluster Management**: Distributed systems and load balancing
- **🚀 Performance Guide**: Benchmarking and optimization tips
- **💡 Best Practices**: Production deployment recommendations
- **📝 Examples**: Working code you can copy and adapt

### API Documentation

```bash
# Open the API reference documentation locally
cargo doc --features codegen --open
```

The API documentation includes:
- **🔧 Full API Reference**: Complete documentation of all public APIs
- **🧪 [Testing Guide](TESTING.md)**: Comprehensive testing and coverage information
- **📊 [Coverage Report](docs/COVERAGE.md)**: Detailed coverage metrics