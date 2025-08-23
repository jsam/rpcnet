# RFC 0001: Service Definition and Code Generation

## Summary

This RFC proposes a service definition language and code generation system for rpcnet that allows developers to define RPC services in a single declarative file and automatically generate both server and client code. This approach is inspired by gRPC's Protocol Buffers but designed specifically for Rust's type system and rpcnet's QUIC-based architecture.

## Motivation

Currently, rpcnet requires manual implementation of both client and server code, including:

- Manual serialization/deserialization of request/response types
- Manual registration of handlers on the server side
- Manual creation of client method calls
- Manual maintenance of type consistency between client and server
- No built-in service discovery or method introspection

This leads to:
- Code duplication between client and server implementations
- Potential type mismatches between client requests and server handlers
- Boilerplate code for common patterns
- Difficulty in maintaining API contracts across versions
- No automatic documentation generation for APIs

## Detailed Design

### Service Definition Language (.rpc files)

We propose a Rust-like syntax for defining RPC services in `.rpc` files:

```rust
// math_service.rpc

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// Request/Response type definitions
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
pub struct DivideRequest {
    pub dividend: f64,
    pub divisor: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DivideResponse {
    pub result: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BatchRequest {
    pub operations: Vec<String>,
    pub values: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BatchResponse {
    pub results: HashMap<String, f64>,
}

// Error types
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MathError {
    DivisionByZero,
    InvalidOperation(String),
    Overflow,
}

// Service definition
#[rpc_service]
pub trait MathService {
    /// Adds two numbers together
    async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError>;
    
    /// Divides two numbers
    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, MathError>;
    
    /// Performs multiple operations in a batch
    async fn batch_calculate(&self, request: BatchRequest) -> Result<BatchResponse, MathError>;
    
    /// Gets server health status
    async fn health_check(&self) -> Result<String, MathError>;
}

// Service configuration
#[rpc_config]
pub struct MathServiceConfig {
    pub max_batch_size: usize = 100,
    pub timeout_seconds: u64 = 30,
    pub enable_compression: bool = true,
}
```

### Code Generation Architecture

The code generation system consists of several components:

#### 1. RPC Compiler (`rpcnet-codegen`)

A standalone binary that parses `.rpc` files and generates Rust code:

```bash
# Install the code generator
cargo install rpcnet-codegen

# Generate code from service definition
rpcnet-codegen --input math_service.rpc --output src/generated/
```

#### 2. Build Integration

Integration with Cargo's build system through `build.rs`:

```rust
// build.rs
fn main() {
    rpcnet_codegen::Builder::new()
        .input("proto/math_service.rpc")
        .output_dir("src/generated")
        .generate()
        .expect("Failed to generate RPC code");
}
```

#### 3. Generated Code Structure

The generator produces several files:

```
src/generated/
├── math_service/
│   ├── mod.rs           # Module exports
│   ├── types.rs         # Request/Response types
│   ├── server.rs        # Server implementation
│   ├── client.rs        # Client implementation
│   └── config.rs        # Configuration types
```

### Generated Server Code

```rust
// Generated in src/generated/math_service/server.rs

use rpcnet::{RpcServer, RpcConfig, RpcError};
use super::types::*;
use async_trait::async_trait;

// Generated trait that users implement
#[async_trait]
pub trait MathServiceHandler: Send + Sync {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError>;
    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, MathError>;
    async fn batch_calculate(&self, request: BatchRequest) -> Result<BatchResponse, MathError>;
    async fn health_check(&self) -> Result<String, MathError>;
}

// Generated server wrapper
pub struct MathServiceServer<H: MathServiceHandler> {
    handler: H,
    config: MathServiceConfig,
    rpc_server: RpcServer,
}

impl<H: MathServiceHandler> MathServiceServer<H> {
    pub fn new(handler: H, config: MathServiceConfig, rpc_config: RpcConfig) -> Self {
        Self {
            handler,
            config,
            rpc_server: RpcServer::new(rpc_config),
        }
    }
    
    pub async fn register_handlers(&mut self) {
        // Generated handler registration
        let handler = Arc::new(self.handler);
        
        self.rpc_server.register("MathService.Add", {
            let handler = handler.clone();
            move |params| {
                let handler = handler.clone();
                async move {
                    let request: AddRequest = bincode::deserialize(&params)
                        .map_err(|e| RpcError::SerializationError(e))?;
                    
                    let response = handler.add(request).await
                        .map_err(|e| RpcError::StreamError(format!("{:?}", e)))?;
                    
                    bincode::serialize(&response)
                        .map_err(RpcError::SerializationError)
                }
            }
        }).await;
        
        // ... similar for other methods
    }
    
    pub async fn serve(mut self, bind_addr: &str) -> Result<(), RpcError> {
        self.register_handlers().await;
        let server = self.rpc_server.bind()?;
        self.rpc_server.start(server).await
    }
}
```

### Generated Client Code

```rust
// Generated in src/generated/math_service/client.rs

use rpcnet::{RpcClient, RpcConfig, RpcError};
use super::types::*;
use std::net::SocketAddr;

// Generated client
pub struct MathServiceClient {
    client: RpcClient,
    config: MathServiceConfig,
}

impl MathServiceClient {
    pub async fn connect(
        addr: SocketAddr, 
        rpc_config: RpcConfig,
        service_config: MathServiceConfig,
    ) -> Result<Self, RpcError> {
        let client = RpcClient::connect(addr, rpc_config).await?;
        Ok(Self {
            client,
            config: service_config,
        })
    }
    
    pub async fn add(&self, request: AddRequest) -> Result<AddResponse, RpcError> {
        let params = bincode::serialize(&request)
            .map_err(RpcError::SerializationError)?;
        
        let response_data = self.client.call("MathService.Add", params).await?;
        
        let response: Result<AddResponse, MathError> = bincode::deserialize(&response_data)
            .map_err(RpcError::SerializationError)?;
        
        response.map_err(|e| RpcError::StreamError(format!("{:?}", e)))
    }
    
    pub async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, RpcError> {
        let params = bincode::serialize(&request)
            .map_err(RpcError::SerializationError)?;
        
        let response_data = self.client.call("MathService.Divide", params).await?;
        
        let response: Result<DivideResponse, MathError> = bincode::deserialize(&response_data)
            .map_err(RpcError::SerializationError)?;
        
        response.map_err(|e| RpcError::StreamError(format!("{:?}", e)))
    }
    
    // ... similar for other methods
}

// Generated convenience methods for common patterns
impl MathServiceClient {
    /// Batch multiple add operations
    pub async fn add_many(&self, requests: Vec<AddRequest>) -> Result<Vec<AddResponse>, RpcError> {
        let mut results = Vec::new();
        for request in requests {
            results.push(self.add(request).await?);
        }
        Ok(results)
    }
    
    /// Add with timeout
    pub async fn add_with_timeout(
        &self, 
        request: AddRequest, 
        timeout: Duration
    ) -> Result<AddResponse, RpcError> {
        tokio::time::timeout(timeout, self.add(request))
            .await
            .map_err(|_| RpcError::Timeout)?
    }
}
```

### Usage Examples

#### Server Implementation

```rust
// src/server.rs

use math_service::server::{MathServiceServer, MathServiceHandler};
use math_service::types::*;
use math_service::config::MathServiceConfig;
use rpcnet::RpcConfig;
use async_trait::async_trait;

struct MathServiceImpl;

#[async_trait]
impl MathServiceHandler for MathServiceImpl {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError> {
        let result = request.a.checked_add(request.b)
            .ok_or(MathError::Overflow)?;
        Ok(AddResponse { result })
    }
    
    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, MathError> {
        if request.divisor == 0.0 {
            return Err(MathError::DivisionByZero);
        }
        Ok(DivideResponse {
            result: request.dividend / request.divisor
        })
    }
    
    async fn batch_calculate(&self, request: BatchRequest) -> Result<BatchResponse, MathError> {
        // Implementation logic
        todo!()
    }
    
    async fn health_check(&self) -> Result<String, MathError> {
        Ok("Service is healthy".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
        .with_key_path("key.pem")
        .with_server_name("localhost");
    
    let service_config = MathServiceConfig {
        max_batch_size: 50,
        timeout_seconds: 60,
        enable_compression: false,
    };
    
    let handler = MathServiceImpl;
    let server = MathServiceServer::new(handler, service_config, rpc_config);
    
    println!("Starting MathService server on 127.0.0.1:8080");
    server.serve("127.0.0.1:8080").await?;
    
    Ok(())
}
```

#### Client Usage

```rust
// src/client.rs

use math_service::client::MathServiceClient;
use math_service::types::*;
use math_service::config::MathServiceConfig;
use rpcnet::RpcConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_config = RpcConfig::new("cert.pem", "127.0.0.1:0")
        .with_server_name("localhost");
    
    let service_config = MathServiceConfig::default();
    
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let client = MathServiceClient::connect(addr, rpc_config, service_config).await?;
    
    // Simple addition
    let add_request = AddRequest { a: 10, b: 20 };
    let add_response = client.add(add_request).await?;
    println!("10 + 20 = {}", add_response.result);
    
    // Division with error handling
    let divide_request = DivideRequest { dividend: 10.0, divisor: 0.0 };
    match client.divide(divide_request).await {
        Ok(response) => println!("Result: {}", response.result),
        Err(e) => println!("Division failed: {}", e),
    }
    
    // Health check
    match client.health_check().await {
        Ok(status) => println!("Server status: {}", status),
        Err(e) => println!("Health check failed: {}", e),
    }
    
    Ok(())
}
```

## Advanced Features

### Streaming Support

Support for streaming requests/responses:

```rust
#[rpc_service]
pub trait StreamingService {
    /// Server streaming: client sends one request, server streams responses
    async fn list_numbers(&self, request: ListRequest) -> impl Stream<Item = NumberResponse>;
    
    /// Client streaming: client streams requests, server sends one response
    async fn sum_numbers(&self, requests: impl Stream<Item = NumberRequest>) -> Result<SumResponse, ServiceError>;
    
    /// Bidirectional streaming
    async fn chat(&self, messages: impl Stream<Item = ChatMessage>) -> impl Stream<Item = ChatMessage>;
}
```

### Middleware Support

```rust
#[rpc_service]
#[middleware(LoggingMiddleware, AuthenticationMiddleware)]
pub trait SecureService {
    #[middleware(RateLimitMiddleware)]
    async fn sensitive_operation(&self, request: SensitiveRequest) -> Result<Response, Error>;
}
```

### Service Discovery

```rust
#[rpc_service]
#[discovery(consul = "math-service", port = 8080)]
pub trait MathService {
    // Service methods
}
```

## Implementation Plan

### Phase 1: Core Code Generation
- [ ] Implement basic `.rpc` file parser
- [ ] Generate simple request/response types
- [ ] Generate basic server and client code
- [ ] Create `rpcnet-codegen` CLI tool

### Phase 2: Advanced Features
- [ ] Add streaming support
- [ ] Implement middleware system
- [ ] Add configuration options
- [ ] Improve error handling

### Phase 3: Tooling and Integration
- [ ] Cargo build integration
- [ ] IDE support (rust-analyzer integration)
- [ ] Documentation generation
- [ ] Service discovery

### Phase 4: Ecosystem
- [ ] Common middleware implementations
- [ ] Testing utilities
- [ ] Performance optimizations
- [ ] Migration tools

## Alternatives Considered

### 1. Use Protocol Buffers
**Pros:** Mature ecosystem, language-neutral, well-documented
**Cons:** Additional build complexity, not idiomatic Rust, binary format

### 2. Use JSON Schema
**Pros:** Human-readable, wide tooling support
**Cons:** Runtime overhead, limited type safety, verbose

### 3. Procedural Macros Only
**Pros:** Compile-time generation, no external tools
**Cons:** Limited flexibility, complex macro code, poor IDE support

## Backwards Compatibility

This RFC is additive and maintains full backward compatibility:
- Existing rpcnet code continues to work unchanged
- Generated code uses the same underlying rpcnet primitives
- Manual implementations can coexist with generated code
- Migration path from manual to generated code is straightforward

## Future Extensions

### OpenAPI Integration
Generate OpenAPI/Swagger documentation from service definitions:

```bash
rpcnet-codegen --input math_service.rpc --openapi-output api-docs.yaml
```

### Cross-Language Support
Potential for generating clients in other languages:

```bash
rpcnet-codegen --input math_service.rpc --language typescript --output clients/ts/
```

### Testing Support
Generate test utilities and mock implementations:

```bash
rpcnet-codegen --input math_service.rpc --generate-tests --generate-mocks
```

## Conclusion

This RFC proposes a comprehensive code generation system that significantly improves the developer experience for rpcnet while maintaining the library's performance and type safety characteristics. The approach provides:

- **Reduced Boilerplate**: Automatic generation eliminates repetitive code
- **Type Safety**: Compile-time guarantees for API contracts
- **Maintainability**: Single source of truth for service definitions
- **Developer Experience**: IDE support, documentation, and tooling
- **Ecosystem Growth**: Foundation for advanced features and integrations

The design balances simplicity with extensibility, providing immediate value while enabling future enhancements that can grow with the community's needs.