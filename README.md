# rpcnet

A blazing-fast RPC library for Rust achieving **130,000+ requests per second** with full QUIC+TLS encryption. Built on the modern QUIC protocol for secure, reliable, and multiplexed communication.

## 🚀 Performance

- **130K+ RPS**: Exceptional throughput with full encryption (exceeds HTTP/1.1 performance!)
- **QUIC Protocol**: Modern transport with connection multiplexing and 0-RTT resumption
- **Optimized**: Custom QUIC limits, efficient buffer management, and minimal allocations
- **Concurrent**: Handle 10,000+ simultaneous streams per connection
- **Low Latency**: Sub-millisecond RTT for local connections

## Features

- **🔒 TLS Security**: Built-in TLS 1.3 encryption and authentication
- **⚡ Async/Await**: Full async support with optimized Tokio runtime
- **📦 Binary Serialization**: Efficient data serialization with bincode
- **🛡️ Type Safety**: Strongly typed RPC calls with compile-time guarantees
- **🔧 Code Generation**: Generate type-safe client and server code from service definitions
- **⏱️ Timeout Handling**: Configurable request timeouts with automatic cleanup
- **🔍 Error Handling**: Comprehensive error types for robust applications
- **📊 Production Ready**: Battle-tested with extensive test coverage

## Installation

### Installing the CLI Tool

The `rpcnet-gen` CLI tool is included with the library. Install it globally:

```bash
# Install from source
cargo install --path . --features codegen

# Or install from crates.io (when published)
cargo install rpcnet --features codegen
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

RpcNet has comprehensive test coverage with unit tests, integration tests, and examples:

```bash
# Run all tests
cargo test

# Generate coverage report
make coverage-html

# Test examples
cargo run --example basic_client_server
```

See [TESTING.md](TESTING.md) for detailed testing information.

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
- **130,000+ requests/second** with full QUIC+TLS encryption
- **Sub-millisecond latency** for local connections
- **10,000+ concurrent streams** per connection
- **Optimized memory usage** with BytesMut buffers

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

**📚 For comprehensive tutorials and documentation, see `cargo doc --open`**

## Documentation

### Quick Documentation Access

```bash
# Open the complete tutorial and API documentation locally
cargo doc --features codegen --open
```

The documentation includes:
- **📚 Complete Tutorial**: Step-by-step guide from basics to advanced patterns
- **🔧 API Reference**: Full documentation of all public APIs
- **🧪 [Testing Guide](TESTING.md)**: Comprehensive testing and coverage information
- **🚀 Performance Guide**: Benchmarking and optimization tips
- **🔨 Code Generation**: Complete guide to the code generation feature
- **💡 Best Practices**: Production deployment recommendations
- **📝 Examples**: Working code you can copy and adapt