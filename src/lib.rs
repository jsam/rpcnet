//! # RpcNet: High-Performance RPC Library
//!
//! A modern, type-safe RPC library built on QUIC protocol with automatic code generation.
//! 
//! RpcNet provides two approaches for building distributed systems:
//! 1. **Generated Code** (recommended) - Type-safe, maintainable services with automatic code generation
//! 2. **Low-level API** - Direct access to the underlying RPC primitives
//!
//! ## Key Features
//!
//! - **🚀 QUIC Protocol**: Modern transport with connection multiplexing and migration
//! - **🔒 TLS Security**: Built-in encryption and certificate-based authentication  
//! - **⚡ Async/Await**: Full async support using Tokio runtime
//! - **🛠️ Code Generation**: Type-safe client/server code from service definitions
//! - **📦 Binary Serialization**: Efficient data serialization using bincode
//! - **⏱️ Timeout Management**: Configurable timeouts for robust operation
//! - **🔧 Error Handling**: Comprehensive error types and recovery mechanisms
//!
//! # Tutorial: Getting Started with RpcNet
//!
//! This tutorial walks you through building RPC services with RpcNet, from basic concepts
//! to advanced patterns. We'll start with code generation (recommended approach) and then
//! cover the low-level API.
//!
//! ## Prerequisites
//!
//! Add RpcNet to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rpcnet = { version = "0.1.0", features = ["codegen"] }
//! async-trait = "0.1"
//! serde = { version = "1.0", features = ["derive"] }
//! tokio = { version = "1.0", features = ["full"] }
//!
//! [build-dependencies]
//! rpcnet = { version = "0.1.0", features = ["codegen"] }
//! ```
//!
//! Install the CLI tool:
//! ```bash
//! cargo install --path . --features codegen
//! ```
//!
//! ## Part 1: Your First Service (Code Generation)
//!
//! ### Step 1: Define Your Service
//!
//! Create a service definition file `greeting.rpc.rs`:
//!
//! ```rust,no_run
//! use rpcnet::service;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! pub struct GreetingRequest {
//!     pub name: String,
//!     pub language: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! pub struct GreetingResponse {
//!     pub message: String,
//!     pub timestamp: u64,
//! }
//!
//! #[derive(Debug)]
//! pub enum GreetingError {
//!     InvalidLanguage,
//!     EmptyName,
//! }
//!
//! #[service]
//! pub trait GreetingService {
//!     async fn greet(&self, request: GreetingRequest) -> Result<GreetingResponse, GreetingError>;
//!     async fn farewell(&self, name: String) -> Result<String, GreetingError>;
//! }
//! ```
//!
//! ```rust,no_run
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct GreetRequest {
//!     pub name: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct GreetResponse {
//!     pub message: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub enum GreetingError {
//!     EmptyName,
//!     InvalidInput(String),
//! }
//!
//! // This attribute is only available with the "codegen" feature
//! #[cfg(feature = "codegen")]
//! #[rpcnet::service]
//! pub trait Greeting {
//!     async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
//! }
//! 
//! // For documentation purposes when codegen is not available
//! #[cfg(not(feature = "codegen"))]
//! pub trait Greeting {
//!     async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
//! }
//! ```
//!
//! ### Step 2: Generate Code
//!
//! ```bash
//! rpcnet-gen --input greeting.rpc.rs --output generated
//! ```
//!
//! This creates:
//! ```text
//! generated/greeting/
//! ├── mod.rs          # Module exports
//! ├── types.rs        # Request/response types
//! ├── server.rs       # Server trait and implementation
//! └── client.rs       # Client implementation
//! ```
//!
//! ### Step 3: Implement the Server
//!
//! ```rust,no_run
//! // Mock the generated module for documentation
//! mod greeting {
//!     use serde::{Serialize, Deserialize};
//!     
//!     #[derive(Serialize, Deserialize, Debug, Clone)]
//!     pub struct GreetRequest { pub name: String }
//!     #[derive(Serialize, Deserialize, Debug, Clone)]
//!     pub struct GreetResponse { pub message: String }
//!     #[derive(Serialize, Deserialize, Debug, Clone)]
//!     pub enum GreetingError { EmptyName, InvalidInput(String) }
//!     
//!     pub mod server {
//!         use super::{GreetRequest, GreetResponse, GreetingError};
//!         
//!         #[async_trait::async_trait]
//!         pub trait GreetingHandler {
//!             async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
//!         }
//!         
//!         pub struct GreetingServer<H> {
//!             handler: H,
//!             config: rpcnet::RpcConfig,
//!         }
//!         
//!         impl<H: GreetingHandler + Send + Sync + 'static> GreetingServer<H> {
//!             pub fn new(handler: H, config: rpcnet::RpcConfig) -> Self {
//!                 Self { handler, config }
//!             }
//!             
//!             pub async fn serve(&self) -> Result<(), Box<dyn std::error::Error>> {
//!                 // Mock implementation for documentation
//!                 Ok(())
//!             }
//!         }
//!     }
//! }
//!
//! use greeting::{GreetRequest, GreetResponse, GreetingError};
//! use greeting::server::{GreetingHandler, GreetingServer};
//! use rpcnet::RpcConfig;
//!
//! struct MyGreetingService;
//!
//! #[async_trait::async_trait]
//! impl GreetingHandler for MyGreetingService {
//!     async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError> {
//!         if request.name.trim().is_empty() {
//!             return Err(GreetingError::EmptyName);
//!         }
//!         
//!         let message = format!("Hello, {}!", request.name);
//!         Ok(GreetResponse { message })
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
//!         .with_key_path("key.pem")
//!         .with_server_name("localhost");
//!     
//!     let server = GreetingServer::new(MyGreetingService, config);
//!     println!("Starting greeting server...");
//!     
//!     server.serve().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Step 4: Implement the Client
//!
//! ```rust,no_run
//! // Mock the generated module for documentation
//! mod greeting {
//!     use serde::{Serialize, Deserialize};
//!     
//!     #[derive(Serialize, Deserialize, Debug, Clone)]
//!     pub struct GreetRequest { pub name: String }
//!     #[derive(Serialize, Deserialize, Debug, Clone)]
//!     pub struct GreetResponse { pub message: String }
//!     
//!     pub mod client {
//!         use super::{GreetRequest, GreetResponse};
//!         use rpcnet::{RpcClient, RpcConfig};
//!         use std::net::SocketAddr;
//!         
//!         pub struct GreetingClient {
//!             client: RpcClient,
//!         }
//!         
//!         impl GreetingClient {
//!             pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, Box<dyn std::error::Error>> {
//!                 let client = RpcClient::connect(addr, config).await?;
//!                 Ok(Self { client })
//!             }
//!             
//!             pub async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, Box<dyn std::error::Error>> {
//!                 // Mock implementation for documentation
//!                 Ok(GreetResponse { message: format!("Hello, {}!", request.name) })
//!             }
//!         }
//!     }
//! }
//!
//! use greeting::GreetRequest;
//! use greeting::client::GreetingClient;
//! use rpcnet::RpcConfig;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
//!         .with_server_name("localhost");
//!     
//!     let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
//!     let client = GreetingClient::connect(server_addr, config).await?;
//!     
//!     let request = GreetRequest { name: "World".to_string() };
//!     let response = client.greet(request).await?;
//!     println!("Response: {}", response.message);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Part 2: Advanced Service Patterns
//!
//! ### Multiple Operations
//!
//! Services can have multiple methods:
//!
//! ```rust,no_run
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct AddRequest { pub a: i64, pub b: i64 }
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct AddResponse { pub result: i64 }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct DivideRequest { pub dividend: f64, pub divisor: f64 }
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct DivideResponse { pub result: f64 }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub enum MathError {
//!     Overflow,
//!     DivisionByZero,
//! }
//!
//! // This attribute is only available with the "codegen" feature
//! #[cfg(feature = "codegen")]
//! #[rpcnet::service]
//! pub trait Calculator {
//!     async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError>;
//!     async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, MathError>;
//! }
//!
//! // For documentation purposes when codegen is not available
//! #[cfg(not(feature = "codegen"))]
//! pub trait Calculator {
//!     async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError>;
//!     async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, MathError>;
//! }
//! ```
//!
//! ### Stateful Services
//!
//! Services can maintain state using `Arc<Mutex<T>>`:
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! struct CounterService {
//!     counter: Arc<Mutex<i64>>,
//! }
//!
//! impl CounterService {
//!     fn new() -> Self {
//!         Self {
//!             counter: Arc::new(Mutex::new(0)),
//!         }
//!     }
//! }
//!
//! // Implement your handler trait here
//! // The counter can be safely accessed across concurrent requests
//! ```
//!
//! ### Binary Data Handling
//!
//! Services can handle binary data efficiently:
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct UploadRequest {
//!     pub filename: String,
//!     pub data: Vec<u8>,  // Binary data
//! }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct UploadResponse {
//!     pub success: bool,
//!     pub bytes_written: usize,
//! }
//! ```
//!
//! ## Part 3: Build Integration
//!
//! ### Automatic Code Generation
//!
//! Add to your `build.rs`:
//!
//! ```rust,no_run
//! fn main() {
//!     println!("cargo:rerun-if-changed=service.rpc.rs");
//!     
//!     #[cfg(feature = "codegen")]
//!     {
//!         rpcnet::codegen::Builder::new()
//!             .input("service.rpc.rs")
//!             .output("src/generated")
//!             .build()
//!             .expect("Failed to generate RPC code");
//!     }
//! }
//! ```
//!
//! ### Project Structure
//!
//! Recommended project layout:
//! ```text
//! my-service/
//! ├── Cargo.toml
//! ├── build.rs              # Code generation
//! ├── service.rpc.rs        # Service definition
//! ├── src/
//! │   ├── main.rs           # Server binary
//! │   ├── client.rs         # Client binary  
//! │   └── generated/        # Auto-generated (gitignored)
//! └── examples/
//!     └── client_example.rs
//! ```
//!
//! ## Part 4: Configuration and Security
//!
//! ### TLS Configuration
//!
//! RpcNet requires TLS certificates. For development, generate self-signed certificates:
//!
//! ```bash
//! # Generate a self-signed certificate (development only)
//! openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
//! ```
//!
//! ### Advanced Configuration
//!
//! ```rust,no_run
//! use rpcnet::RpcConfig;
//! use std::time::Duration;
//!
//! let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
//!     .with_key_path("key.pem")
//!     .with_server_name("myservice.example.com")
//!     .with_keep_alive_interval(Duration::from_secs(30));
//! ```
//!
//! ## Part 5: Error Handling and Best Practices
//!
//! ### Comprehensive Error Handling
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub enum ServiceError {
//!     /// Input validation failed
//!     ValidationError(String),
//!     /// External service unavailable
//!     ServiceUnavailable,
//!     /// Rate limit exceeded
//!     RateLimitExceeded,
//!     /// Internal server error
//!     InternalError(String),
//! }
//! ```
//!
//! ### Best Practices
//!
//! 1. **Service Design**:
//!    - Keep interfaces focused and cohesive
//!    - Use descriptive names for operations and types
//!    - Design for forward compatibility
//!
//! 2. **Error Handling**:
//!    - Define specific error types for your domain
//!    - Use descriptive error messages
//!    - Handle timeouts and network errors gracefully
//!
//! 3. **Testing**:
//!    - Test both success and error cases
//!    - Use integration tests for end-to-end validation
//!    - Mock external dependencies
//!
//! 4. **Performance**:
//!    - Reuse connections when possible
//!    - Use appropriate timeouts
//!    - Consider chunking for large data transfers
//!
//! ## Part 6: Examples and Patterns
//!
//! The `examples/` directory contains examples for both approaches:
//!
//! ### Basic Examples (Low-Level API)
//! These work immediately without setup:
//! - **`basic_server/client`**: Simple RPC communication
//! - **`simple_echo_server/client`**: Text and binary data handling
//!
//! ```bash
//! # Try these first - no setup required
//! cargo run --example basic_server
//! cargo run --example simple_echo_server
//! ```
//!
//! ### Advanced Examples (Generated Code)
//! Complete, self-contained examples with code generation:
//! - **`basic_greeting/`**: Simple request/response service
//! - **`echo/`**: Binary data handling and multiple methods  
//! - **`calculator/`**: Mathematical operations with error handling
//! - **`file_transfer/`**: Chunked operations and stateful services
//! - **`concurrent_demo/`**: Concurrent operations and shared state
//!
//! ```bash
//! # Generated code examples
//! cargo run --example basic_greeting_server --features codegen
//! cargo run --example basic_greeting_client --features codegen
//! ```
//!
//! # Part 7: Low-Level API (Advanced)
//!
//! For cases where you need direct control over RPC operations, RpcNet provides
//! a low-level API that works with raw bytes and string method names.
//!
//! ## Low-Level Server
//!
//! ```rust,no_run
//! use rpcnet::{RpcServer, RpcConfig, RpcError};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct CalculateRequest { a: i32, b: i32 }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
//!         .with_key_path("key.pem")
//!         .with_server_name("localhost");
//!
//!     let mut server = RpcServer::new(config);
//!
//!     // Register method with string name and raw bytes
//!     server.register("calculate", |params| async move {
//!         let request: CalculateRequest = bincode::deserialize(&params)
//!             .map_err(RpcError::SerializationError)?;
//!         
//!         let result = request.a + request.b;
//!         Ok(bincode::serialize(&result)?)
//!     }).await;
//!
//!     let quic_server = server.bind()?;
//!     server.start(quic_server).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Low-Level Client
//!
//! ```rust,no_run
//! use rpcnet::{RpcClient, RpcConfig};
//! use serde::{Serialize, Deserialize};
//! use std::net::SocketAddr;
//!
//! #[derive(Serialize, Deserialize)]
//! struct CalculateRequest { a: i32, b: i32 }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
//!         .with_server_name("localhost");
//!     
//!     let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
//!     let client = RpcClient::connect(server_addr, config).await?;
//!     
//!     // Manual serialization and method name
//!     let request = CalculateRequest { a: 10, b: 20 };
//!     let params = bincode::serialize(&request)?;
//!     let response = client.call("calculate", params).await?;
//!     let result: i32 = bincode::deserialize(&response)?;
//!     
//!     println!("Result: {}", result);
//!     Ok(())
//! }
//! ```
//!
//! ## When to Use Low-Level API
//!
//! Consider the low-level API when you need:
//! - Dynamic method dispatch at runtime
//! - Custom serialization formats
//! - Integration with existing non-Rust systems  
//! - Maximum control over the RPC protocol
//!
//! For most use cases, the generated code approach is recommended for its
//! type safety, maintainability, and ease of use.
//!
//! ## Quick Start
//!
//! ### Setting up a Server
//!
//! ```rust,no_run
//! use rpcnet::{RpcServer, RpcConfig, RpcError};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct CalculateRequest {
//!     a: i32,
//!     b: i32,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure the server with TLS certificates
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
//!         .with_key_path("key.pem")
//!         .with_server_name("localhost");
//!
//!     let mut server = RpcServer::new(config);
//!
//!     // Register an RPC method handler
//!     server.register("calculate", |params| async move {
//!         let request: CalculateRequest = bincode::deserialize(&params)
//!             .map_err(RpcError::SerializationError)?;
//!         
//!         let result = request.a + request.b;
//!         Ok(bincode::serialize(&result)?)
//!     }).await;
//!
//!     // Start the server
//!     let quic_server = server.bind()?;
//!     server.start(quic_server).await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Connecting a Client
//!
//! ```rust,no_run
//! use rpcnet::{RpcClient, RpcConfig};
//! use serde::{Serialize, Deserialize};
//! use std::net::SocketAddr;
//!
//! #[derive(Serialize, Deserialize)]
//! struct CalculateRequest { a: i32, b: i32 }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
//!         .with_server_name("localhost");
//!
//!     let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
//!     let client = RpcClient::connect(server_addr, config).await?;
//!
//!     // Prepare request data
//!     let request = CalculateRequest { a: 10, b: 20 };
//!     let params = bincode::serialize(&request)?;
//!
//!     // Make the RPC call
//!     let response = client.call("calculate", params).await?;
//!     let result: i32 = bincode::deserialize(&response)?;
//!     
//!     println!("Result: {}", result); // Result: 30
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The library follows a straightforward client-server architecture:
//!
//! - **RpcServer**: Accepts incoming QUIC connections and dispatches RPC calls to registered handlers
//! - **RpcClient**: Connects to servers and makes RPC calls over QUIC streams  
//! - **RpcConfig**: Manages TLS certificates, network addresses, and connection parameters
//! - **Handler Functions**: Async closures that process RPC requests and return responses
//!
//! Each RPC call uses its own bidirectional QUIC stream, providing natural isolation
//! and allowing concurrent operations without blocking. The underlying QUIC connection
//! handles multiplexing, flow control, and network-level optimizations automatically.
//!
//! ## Error Handling
//!
//! rpcnet provides comprehensive error handling through the [`RpcError`] enum, covering
//! common scenarios like network failures, timeouts, serialization issues, and
//! configuration problems. All operations return Results that should be properly
//! handled in production code.
//!
//! ## Security Considerations
//!
//! - TLS certificates must be properly configured for secure communication
//! - Certificate validation ensures clients connect to trusted servers
//! - Consider certificate rotation and management in production deployments
//! - Network-level security (firewalls, VPNs) may be needed depending on deployment

use bytes::BytesMut;
use futures::{Stream, StreamExt};
use s2n_quic::{Client, client::Connect};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use thiserror::Error;
use tokio::sync::RwLock;

// Code generation module
#[cfg(feature = "codegen")]
pub mod codegen;

/// Default timeout duration for RPC operations.
/// 
/// In production builds, RPC calls will timeout after 30 seconds if no response
/// is received. This provides a reasonable balance between allowing complex
/// operations to complete while preventing indefinite hangs.
/// 
/// In test builds, the timeout is reduced to 2 seconds to make test execution
/// faster and detect timeout scenarios quickly.
#[cfg(not(test))]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout duration for RPC operations in tests.
/// 
/// Shortened timeout for faster test execution and reliable timeout testing.
#[cfg(test)]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// Comprehensive error type for all RPC operations.
///
/// This enum covers all possible error conditions that can occur during RPC
/// communication, from network-level failures to application-level issues.
/// Each variant provides detailed context to help with debugging and error
/// handling in production systems.
///
/// # Examples
///
/// ```rust
/// use rpcnet::RpcError;
///
/// // Network-related errors
/// let conn_err = RpcError::ConnectionError("Server unreachable".to_string());
/// let timeout_err = RpcError::Timeout;
///
/// // Configuration errors  
/// let config_err = RpcError::ConfigError("Invalid certificate path".to_string());
///
/// // Application errors
/// let method_err = RpcError::UnknownMethod("calculate_advanced".to_string());
/// ```
#[derive(Debug, Error)]
pub enum RpcError {
    /// Network connection failed or was lost.
    ///
    /// This occurs when the QUIC connection cannot be established or is
    /// unexpectedly closed. Common causes include network outages, server
    /// unavailability, or firewall blocking.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// QUIC stream operation failed.
    ///
    /// Individual streams within a QUIC connection can fail independently.
    /// This includes stream creation failures, send/receive errors, and
    /// stream closure issues. The connection may still be valid.
    #[error("Stream error: {0}")]
    StreamError(String),

    /// TLS handshake or certificate validation failed.
    ///
    /// This indicates problems with TLS setup, certificate verification,
    /// or cryptographic operations. Check certificate paths, validity,
    /// and server name configuration.
    #[error("TLS error: {0}")]
    TlsError(String),

    /// Binary serialization or deserialization failed.
    ///
    /// This occurs when request/response data cannot be properly encoded
    /// or decoded using bincode. Usually indicates type mismatches between
    /// client and server or corrupted data.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    /// RPC operation exceeded the configured timeout.
    ///
    /// The server did not respond within the [`DEFAULT_TIMEOUT`] period.
    /// This could indicate server overload, network delays, or long-running
    /// operations that need timeout adjustments.
    #[error("Request timeout")]
    Timeout,

    /// Server does not have a handler for the requested method.
    ///
    /// The method name in the RPC call does not match any registered
    /// handler on the server. Check method names for typos and ensure
    /// all required handlers are properly registered.
    #[error("Unknown method: {0}")]
    UnknownMethod(String),

    /// Configuration parameter is invalid or missing.
    ///
    /// This includes invalid file paths, malformed addresses, missing
    /// certificates, or other configuration issues that prevent proper
    /// initialization of the RPC system.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Underlying I/O operation failed.
    ///
    /// File system operations, network socket operations, or other
    /// system-level I/O failed. Check file permissions, disk space,
    /// and system resource availability.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Internal RPC request structure.
///
/// This structure represents an RPC call being sent from client to server.
/// It contains all the necessary information to identify, route, and process
/// the request. The structure is serialized using bincode for efficient
/// network transmission.
///
/// You typically don't need to create these directly - the [`RpcClient`]
/// handles request creation automatically when you call [`RpcClient::call`].
///
/// # Fields
///
/// - `id`: Unique identifier for request/response matching
/// - `method`: Name of the RPC method to invoke  
/// - `params`: Serialized parameters for the method call
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    id: u64,
    method: String,
    params: Vec<u8>,
}

impl RpcRequest {
    /// Creates a new RPC request.
    ///
    /// This is primarily used internally by the client. The `id` should be
    /// unique per connection to enable proper request/response matching.
    ///
    /// # Parameters
    ///
    /// - `id`: Unique request identifier
    /// - `method`: Name of the remote method to call
    /// - `params`: Serialized parameters for the method
    pub fn new(id: u64, method: String, params: Vec<u8>) -> Self {
        Self { id, method, params }
    }

    /// Returns the request ID.
    ///
    /// Used for matching responses to requests in concurrent scenarios.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the method name.
    ///
    /// This is used by the server to route the request to the appropriate handler.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the serialized parameters.
    ///
    /// These are the binary-encoded parameters that will be passed to
    /// the handler function on the server side.
    pub fn params(&self) -> &[u8] {
        &self.params
    }
}

/// Internal RPC response structure.
///
/// This structure represents the response sent from server back to client
/// after processing an RPC request. It contains either successful result data
/// or error information, but never both.
///
/// Like [`RpcRequest`], you typically don't create these directly - the
/// server framework handles response creation and the client automatically
/// processes responses when calling [`RpcClient::call`].
///
/// # Fields
///
/// - `id`: Request ID this response corresponds to
/// - `result`: Successful response data (mutually exclusive with error)
/// - `error`: Error message if the operation failed
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    id: u64,
    result: Option<Vec<u8>>,
    error: Option<String>,
}

impl RpcResponse {
    /// Creates a new RPC response.
    ///
    /// Either `result` or `error` should be provided, but not both.
    /// This is used internally by the server framework.
    ///
    /// # Parameters
    ///
    /// - `id`: Request ID this response corresponds to
    /// - `result`: Successful response data, if any
    /// - `error`: Error message, if the operation failed
    pub fn new(id: u64, result: Option<Vec<u8>>, error: Option<String>) -> Self {
        Self { id, result, error }
    }

    /// Creates a response from a Result.
    ///
    /// This is a convenience method that converts a standard Rust Result
    /// into the appropriate RpcResponse with either success data or error message.
    ///
    /// # Parameters
    ///
    /// - `id`: Request ID this response corresponds to
    /// - `result`: Result from handler execution
    pub fn from_result(id: u64, result: Result<Vec<u8>, RpcError>) -> Self {
        match result {
            Ok(data) => Self::new(id, Some(data), None),
            Err(e) => Self::new(id, None, Some(e.to_string())),
        }
    }

    /// Returns the request ID this response corresponds to.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the successful result data, if any.
    ///
    /// This will be `None` if the operation failed or returned an error.
    pub fn result(&self) -> Option<&Vec<u8>> {
        self.result.as_ref()
    }

    /// Returns the error message, if any.
    ///
    /// This will be `None` if the operation succeeded.
    pub fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }
}

/// Configuration for RPC client and server connections.
///
/// This structure holds all the necessary configuration parameters for establishing
/// secure QUIC connections. It uses a builder pattern for convenient configuration
/// and supports both client and server use cases.
///
/// # TLS Configuration
///
/// Both client and server require TLS certificates for secure communication:
/// - **Server**: Requires both certificate (`cert_path`) and private key (`key_path`)
/// - **Client**: Only requires the certificate for server verification
///
/// # Examples
///
/// ```rust
/// use rpcnet::RpcConfig;
/// use std::time::Duration;
///
/// // Basic server configuration
/// let server_config = RpcConfig::new("server.pem", "127.0.0.1:8080")
///     .with_key_path("server-key.pem")
///     .with_server_name("myapp.example.com");
///
/// // Client configuration with custom keep-alive
/// let client_config = RpcConfig::new("ca-cert.pem", "127.0.0.1:0")
///     .with_server_name("myapp.example.com")
///     .with_keep_alive_interval(Duration::from_secs(60));
/// ```
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Path to the TLS certificate file.
    /// 
    /// For servers, this is their own certificate that clients will verify.
    /// For clients, this is typically the CA certificate or server certificate
    /// used to verify the server's identity.
    pub cert_path: PathBuf,
    
    /// Path to the private key file (required for servers).
    /// 
    /// This should correspond to the certificate in `cert_path`. Clients
    /// typically don't need to set this unless using client certificates.
    pub key_path: Option<PathBuf>,
    
    /// Server name for TLS verification.
    /// 
    /// This must match the common name or subject alternative name in the
    /// server's certificate. Used by clients to verify server identity.
    pub server_name: String,
    
    /// Network address to bind to (servers) or connect from (clients).
    /// 
    /// For servers, this specifies the interface and port to listen on.
    /// For clients, use "127.0.0.1:0" to bind to any available local port.
    pub bind_address: String,
    
    /// Keep-alive interval for QUIC connections.
    /// 
    /// If set, enables periodic keep-alive packets to maintain connection
    /// state through NATs and firewalls. Recommended for long-lived connections.
    pub keep_alive_interval: Option<Duration>,
}

impl RpcConfig {
    /// Creates a new configuration with default settings.
    ///
    /// This initializes a configuration with sensible defaults:
    /// - Server name: "localhost"
    /// - Keep-alive: 30 seconds
    /// - No private key (must be set for servers)
    ///
    /// # Parameters
    ///
    /// - `cert_path`: Path to TLS certificate file
    /// - `bind_address`: Network address (e.g., "127.0.0.1:8080" for servers, "127.0.0.1:0" for clients)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::RpcConfig;
    ///
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080");
    /// ```
    pub fn new<P: Into<PathBuf>>(cert_path: P, bind_address: impl Into<String>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: None,
            server_name: "localhost".to_string(),
            bind_address: bind_address.into(),
            keep_alive_interval: Some(Duration::from_secs(30)),
        }
    }

    /// Sets the private key path (required for servers).
    ///
    /// The private key must correspond to the certificate specified in the constructor.
    /// This is typically required for servers but optional for clients.
    ///
    /// # Parameters
    ///
    /// - `key_path`: Path to the private key file
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::RpcConfig;
    ///
    /// let config = RpcConfig::new("server.pem", "0.0.0.0:8080")
    ///     .with_key_path("server-key.pem");
    /// ```
    pub fn with_key_path<P: Into<PathBuf>>(mut self, key_path: P) -> Self {
        self.key_path = Some(key_path.into());
        self
    }

    /// Sets the server name for TLS verification.
    ///
    /// This name must match the common name or a subject alternative name
    /// in the server's certificate. Critical for proper TLS verification.
    ///
    /// # Parameters
    ///
    /// - `server_name`: Server hostname for TLS verification
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::RpcConfig;
    ///
    /// let config = RpcConfig::new("ca-cert.pem", "127.0.0.1:0")
    ///     .with_server_name("api.myservice.com");
    /// ```
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = server_name.into();
        self
    }

    /// Sets the keep-alive interval for connections.
    ///
    /// When set, the QUIC connection will send periodic keep-alive packets
    /// to maintain connection state through NATs and firewalls. This is
    /// recommended for long-lived connections.
    ///
    /// # Parameters
    ///
    /// - `interval`: Time between keep-alive packets
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::RpcConfig;
    /// use std::time::Duration;
    ///
    /// // Keep-alive every 60 seconds
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
    ///     .with_keep_alive_interval(Duration::from_secs(60));
    ///
    /// // Disable keep-alive
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:0")
    ///     .with_keep_alive_interval(Duration::ZERO);
    /// ```
    pub fn with_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.keep_alive_interval = Some(interval);
        self
    }
}

/// Internal type alias for RPC handler functions.
///
/// This represents the boxed async closure type used internally to store
/// registered RPC handlers. Each handler takes serialized parameters and
/// returns a future that resolves to either response data or an error.
///
/// You don't need to work with this type directly - use the [`RpcServer::register`]
/// method which accepts regular async closures and handles the boxing automatically.
///
/// # Handler Function Signature
///
/// ```rust,no_run
/// # use std::future::Future;
/// # use rpcnet::RpcError;
/// # type HandlerFn = Box<dyn
/// Fn(Vec<u8>) -> Box<dyn Future<Output = Result<Vec<u8>, RpcError>> + Send>
/// # + Send + Sync>;
/// ```
///
/// Where:
/// - Input `Vec<u8>`: Serialized request parameters
/// - Output `Vec<u8>`: Serialized response data  
/// - `RpcError`: Any error that occurred during processing
type AsyncHandlerFn = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RpcError>> + Send>>
        + Send
        + Sync,
>;

/// Type alias for streaming RPC handlers.
/// 
/// Streaming handlers receive a stream of request data and return a stream of response data.
/// This enables bidirectional streaming communication between client and server.
type AsyncStreamingHandlerFn = Box<
    dyn Fn(
            Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>,
        ) -> Pin<Box<dyn Future<Output = Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>> + Send>>
        + Send
        + Sync,
>;

/// RPC server that handles incoming QUIC connections and dispatches requests.
///
/// The server accepts incoming QUIC connections, receives RPC requests over
/// bidirectional streams, routes them to registered handler functions, and
/// sends responses back to clients. It supports concurrent request handling
/// and automatic connection management.
///
/// # Architecture
///
/// - Each client connection runs in its own task
/// - Each RPC request uses a dedicated bidirectional QUIC stream
/// - Handler functions are called asynchronously and concurrently
/// - Responses are sent back over the same stream used for the request
///
/// # Example Usage
///
/// ```rust,no_run
/// use rpcnet::{RpcServer, RpcConfig, RpcError};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct AddRequest { a: i32, b: i32 }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
///         .with_key_path("key.pem")
///         .with_server_name("localhost");
///
///     let mut server = RpcServer::new(config);
///
///     // Register a handler for the "add" method
///     server.register("add", |params| async move {
///         let req: AddRequest = bincode::deserialize(&params)
///             .map_err(RpcError::SerializationError)?;
///         let result = req.a + req.b;
///         Ok(bincode::serialize(&result)?)
///     }).await;
///
///     // Start the server (this blocks)
///     let quic_server = server.bind()?;
///     server.start(quic_server).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct RpcServer {
    /// Map of method names to their handler functions.
    /// 
    /// Protected by RwLock to allow concurrent reads during request handling
    /// while still supporting dynamic handler registration.
    pub handlers: Arc<RwLock<HashMap<String, AsyncHandlerFn>>>,
    
    /// Map of streaming method names to their streaming handler functions.
    /// 
    /// Protected by RwLock to allow concurrent reads during request handling
    /// while still supporting dynamic handler registration.
    pub streaming_handlers: Arc<RwLock<HashMap<String, AsyncStreamingHandlerFn>>>,
    
    /// The local socket address the server is bound to, if any.
    /// 
    /// This is populated when [`bind`](RpcServer::bind) is called and can be
    /// used to discover the actual port when binding to port 0.
    pub socket_addr: Option<SocketAddr>,
    
    /// Server configuration including TLS settings and network parameters.
    pub config: RpcConfig,
}

impl RpcServer {
    /// Creates a new RPC server with the given configuration.
    ///
    /// The server starts with no registered handlers. You must register at least
    /// one handler using [`register`](RpcServer::register) before starting the server.
    ///
    /// # Parameters
    ///
    /// - `config`: Server configuration including TLS certificates and network settings
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::{RpcServer, RpcConfig};
    ///
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
    ///     .with_key_path("key.pem");
    /// let server = RpcServer::new(config);
    /// ```
    pub fn new(config: RpcConfig) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            streaming_handlers: Arc::new(RwLock::new(HashMap::new())),
            socket_addr: None,
            config,
        }
    }

    /// Registers an async RPC method handler.
    ///
    /// This method allows you to register a handler function for a specific RPC method name.
    /// The handler receives the raw serialized parameters and must return serialized response data.
    /// Multiple handlers can be registered for different methods.
    ///
    /// # Handler Function Requirements
    ///
    /// - Must be `Send + Sync + 'static` for thread safety
    /// - Takes `Vec<u8>` (serialized parameters) as input
    /// - Returns a Future that resolves to `Result<Vec<u8>, RpcError>`
    /// - Should handle deserialization of input and serialization of output
    ///
    /// # Parameters
    ///
    /// - `method`: The RPC method name that clients will call
    /// - `handler`: Async function that processes requests for this method
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcServer, RpcConfig, RpcError};
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct EchoRequest { message: String }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
    ///     .with_key_path("key.pem");
    /// let server = RpcServer::new(config);
    ///
    /// // Register an echo handler
    /// server.register("echo", |params| async move {
    ///     let request: EchoRequest = bincode::deserialize(&params)
    ///         .map_err(RpcError::SerializationError)?;
    ///     
    ///     println!("Echoing: {}", request.message);
    ///     Ok(bincode::serialize(&request.message)?)
    /// }).await;
    ///
    /// // Register a handler that can fail
    /// server.register("divide", |params| async move {
    ///     let (a, b): (f64, f64) = bincode::deserialize(&params)
    ///         .map_err(RpcError::SerializationError)?;
    ///     
    ///     if b == 0.0 {
    ///         return Err(RpcError::StreamError("Division by zero".to_string()));
    ///     }
    ///     
    ///     Ok(bincode::serialize(&(a / b))?)
    /// }).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, RpcError>> + Send + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |params: Vec<u8>| {
                Box::pin(handler(params)) as Pin<Box<dyn Future<Output = _> + Send>>
            }),
        );
    }

    /// Registers a streaming RPC method handler.
    ///
    /// This method allows you to register a handler function for streaming RPC operations.
    /// Streaming handlers receive a stream of requests and return a stream of responses,
    /// enabling efficient bulk operations and real-time communication.
    ///
    /// # Parameters
    ///
    /// - `method`: The name of the RPC method to handle (e.g., "stream_process")
    /// - `handler`: An async function that takes a request stream and returns a response stream
    ///
    /// # Handler Function Signature
    ///
    /// The handler function should have the signature:
    /// ```rust,no_run
    /// # use futures::Stream;
    /// # use rpcnet::RpcError;
    /// # use std::pin::Pin;
    /// # type StreamHandler = Box<dyn
    /// # Fn(Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>) 
    /// #     -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>
    /// # >;
    /// ```
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rpcnet::{RpcServer, RpcConfig, RpcError};
    /// use futures::{Stream, StreamExt, stream};
    /// use async_stream::stream;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
    ///     .with_key_path("key.pem");
    /// let server = RpcServer::new(config);
    ///
    /// // Register a streaming echo handler
    /// server.register_streaming("echo_stream", |request_stream| async move {
    ///     let mut request_stream = request_stream;
    ///     Box::pin(stream! {
    ///         while let Some(data) = request_stream.next().await {
    ///             // Echo each request back
    ///             yield Ok(data);
    ///         }
    ///     }) as std::pin::Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>
    /// }).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_streaming<F, Fut, S>(&self, method: &str, handler: F)
    where
        F: Fn(Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = S> + Send + 'static,
        S: Stream<Item = Result<Vec<u8>, RpcError>> + Send + 'static,
    {
        let mut handlers = self.streaming_handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |request_stream| {
                let handler = handler.clone();
                Box::pin(async move {
                    let response_stream = handler(request_stream).await;
                    Box::pin(response_stream) as Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>
                })
            }),
        );
    }

    /// Starts the server and begins accepting client connections.
    ///
    /// This method runs the main server loop, accepting incoming QUIC connections
    /// and spawning tasks to handle each client. It will block indefinitely until
    /// the server is shut down or an unrecoverable error occurs.
    ///
    /// # Server Operation
    ///
    /// 1. Accepts new QUIC connections from clients
    /// 2. For each connection, spawns a task to handle all streams from that client
    /// 3. For each stream, spawns another task to handle the individual RPC request
    /// 4. Routes requests to the appropriate registered handlers
    /// 5. Sends responses back over the same stream
    ///
    /// # Parameters
    ///
    /// - `server`: The bound QUIC server from [`bind`](RpcServer::bind)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcServer, RpcConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
    ///     .with_key_path("key.pem");
    /// let mut server = RpcServer::new(config);
    ///
    /// // Register handlers first
    /// server.register("ping", |_| async move {
    ///     Ok(b"pong".to_vec())
    /// }).await;
    ///
    /// // Bind and start the server
    /// let quic_server = server.bind()?;
    /// println!("Server listening on: {:?}", server.socket_addr);
    /// 
    /// // This blocks until the server is shutdown
    /// server.start(quic_server).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&mut self, mut server: s2n_quic::Server) -> Result<(), RpcError> {

        while let Some(mut connection) = server.accept().await {
            let handlers = self.handlers.clone();
            let streaming_handlers = self.streaming_handlers.clone();
            
            tokio::spawn(async move {
                // For each accepted connection, keep accepting streams:
                while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                    let handlers = handlers.clone();
                    let streaming_handlers = streaming_handlers.clone();
                    
                    tokio::spawn(async move {
                        let mut request_data = Vec::with_capacity(8192);
                        
                        while let Ok(Some(data)) = stream.receive().await {
                            request_data.extend_from_slice(&data);
                            
                            // First, try to parse as regular RPC request (original behavior)
                            if let Ok(request) = bincode::deserialize::<RpcRequest>(&request_data) {
                                let handlers = handlers.read().await;
                                let response = match handlers.get(request.method()) {
                                    Some(handler) => {
                                        let result = handler(request.params().to_vec()).await;
                                        RpcResponse::from_result(request.id(), result)
                                    }
                                    None => RpcResponse::new(
                                        request.id(),
                                        None,
                                        Some(format!("Unknown method: {}", request.method())),
                                    ),
                                };
                                if let Ok(response_data) = bincode::serialize(&response) {
                                    let _ = stream.send(response_data.into()).await;
                                }
                                break; // Handle one request per stream
                            }
                            
                            // If regular RPC parsing fails and we have enough data, check for streaming protocol
                            if request_data.len() >= 4 {
                                let method_len = u32::from_le_bytes([
                                    request_data[0], request_data[1], request_data[2], request_data[3]
                                ]) as usize;
                                
                                // Validate method length is reasonable (prevent huge allocations)
                                if method_len > 0 && method_len < 1024 && request_data.len() >= 4 + method_len {
                                    if let Ok(method_name) = std::str::from_utf8(&request_data[4..4 + method_len]) {
                                        // Check if this is a known streaming method
                                        let streaming_handlers_ref = streaming_handlers.read().await;
                                        if streaming_handlers_ref.contains_key(method_name) {
                                            // This is a streaming request - handle it differently
                                            drop(streaming_handlers_ref); // Release the read lock
                                            
                                            // Create stream with remaining data after method name
                                            let remaining_data = request_data[4 + method_len..].to_owned();
                                            let stream_arc = std::sync::Arc::new(tokio::sync::Mutex::new(stream));
                                            let request_stream = Self::create_request_stream_with_initial_data(stream_arc.clone(), remaining_data);
                                            
                                            let streaming_handlers_ref = streaming_handlers.read().await;
                                            if let Some(handler) = streaming_handlers_ref.get(method_name) {
                                                let response_stream = handler(request_stream).await;
                                                Self::send_response_stream(stream_arc, response_stream).await;
                                            }
                                            return; // Exit properly instead of break
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }

        Ok(())
    }
    
    /// Creates a stream of incoming requests from the QUIC stream with initial data.
    fn create_request_stream_with_initial_data(
        stream: std::sync::Arc<tokio::sync::Mutex<s2n_quic::stream::BidirectionalStream>>, 
        initial_data: Vec<u8>
    ) -> Pin<Box<dyn Stream<Item = Vec<u8>> + Send>> {
        Box::pin(
        async_stream::stream! {
            let mut buffer = BytesMut::with_capacity(8192 + initial_data.len());
            buffer.extend_from_slice(&initial_data);
            
            loop {
                // First try to parse any complete messages from existing buffer
                while buffer.len() >= 4 {
                    let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
                    
                    if len == 0 {
                        // End of stream marker
                        return;
                    }
                    
                    if buffer.len() >= 4 + len {
                        // We have a complete message
                        let message_data = buffer.split_to(4 + len);
                        let request_data = message_data[4..].to_owned();
                        yield request_data;
                    } else {
                        // Need more data
                        break;
                    }
                }
                
                // If we need more data, receive from stream
                let chunk = {
                    let mut stream_guard = stream.lock().await;
                    stream_guard.receive().await
                };
                
                if let Ok(Some(chunk)) = chunk {
                    buffer.extend_from_slice(&chunk);
                } else {
                    // Connection closed or error
                    break;
                }
            }
        }
        )
    }

    /// Creates a stream of incoming requests from the QUIC stream.
    fn create_request_stream(stream: std::sync::Arc<tokio::sync::Mutex<s2n_quic::stream::BidirectionalStream>>) -> Pin<Box<dyn Stream<Item = Vec<u8>> + Send>> {
        Box::pin(
        async_stream::stream! {
            let mut buffer = BytesMut::with_capacity(8192);
            
            loop {
                let chunk = {
                    let mut stream_guard = stream.lock().await;
                    stream_guard.receive().await
                };
                
                if let Ok(Some(chunk)) = chunk {
                buffer.extend_from_slice(&chunk);
                
                // Parse length-prefixed messages
                while buffer.len() >= 4 {
                    let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
                    
                    if len == 0 {
                        // End of stream marker
                        return;
                    }
                    
                    if buffer.len() >= 4 + len {
                        // We have a complete message
                        let message_data = buffer.split_to(4 + len);
                        let request_data = message_data[4..].to_owned();
                        yield request_data;
                    } else {
                        // Need more data
                        break;
                    }
                }
                } else {
                    // Connection closed or error
                    break;
                }
            }
        })
    }
    
    /// Sends a stream of responses back over the QUIC stream.
    async fn send_response_stream(
        stream: std::sync::Arc<tokio::sync::Mutex<s2n_quic::stream::BidirectionalStream>>,
        mut response_stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>,
    ) {
        while let Some(response_result) = response_stream.next().await {
            match response_result {
                Ok(response_data) => {
                    let data_len = (response_data.len() as u32).to_le_bytes();
                    let mut stream_guard = stream.lock().await;
                    if stream_guard.send([&data_len[..], &response_data].concat().into()).await.is_err() {
                        break;
                    }
                }
                Err(_) => {
                    // Send error and continue
                    let error_data = b"Error processing request";
                    let data_len = (error_data.len() as u32).to_le_bytes();
                    let mut stream_guard = stream.lock().await;
                    if stream_guard.send([&data_len[..], error_data].concat().into()).await.is_err() {
                        break;
                    }
                }
            }
        }
        
        // Send end-of-stream marker
        let mut stream_guard = stream.lock().await;
        let _ = stream_guard.send(vec![0, 0, 0, 0].into()).await;
    }

    /// Binds the server to the configured address and prepares it for accepting connections.
    ///
    /// This method sets up the QUIC server with TLS configuration and binds it to
    /// the network address specified in the config. It must be called before
    /// [`start`](RpcServer::start).
    ///
    /// # TLS Requirements
    ///
    /// The server requires both a certificate and private key file for TLS operation.
    /// The certificate file should contain the server's TLS certificate, and the
    /// key file should contain the corresponding private key.
    ///
    /// # Network Binding
    ///
    /// The server binds to the address specified in `config.bind_address`. Use
    /// "0.0.0.0:port" to listen on all interfaces, or "127.0.0.1:port" for localhost only.
    /// Using port 0 will bind to any available port - check `socket_addr` after binding
    /// to see the actual port assigned.
    ///
    /// # Returns
    ///
    /// Returns a bound QUIC server ready to be passed to [`start`](RpcServer::start).
    /// The server's local address is also stored in `socket_addr` for reference.
    ///
    /// # Errors
    ///
    /// - [`RpcError::ConfigError`] if required configuration is missing or invalid
    /// - [`RpcError::TlsError`] if TLS setup fails
    /// - [`RpcError::IoError`] if network binding fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcServer, RpcConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RpcConfig::new("server.pem", "127.0.0.1:0")  // Port 0 = any port
    ///     .with_key_path("server-key.pem")
    ///     .with_server_name("localhost");
    ///
    /// let mut server = RpcServer::new(config);
    /// let quic_server = server.bind()?;
    ///
    /// // Check what port was actually assigned
    /// println!("Server bound to: {:?}", server.socket_addr);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bind(&mut self) -> Result<s2n_quic::Server, RpcError> {
        let key_path =
            self.config.key_path.as_ref().ok_or_else(|| {
                RpcError::ConfigError("Server key path not configured".to_string())
            })?;

        // Create optimized limits for high-performance RPC
        let limits = s2n_quic::provider::limits::Limits::new()
            // Increase stream limits for high concurrency
            .with_max_open_local_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set stream limits: {:?}", e)))?
            .with_max_open_remote_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set stream limits: {:?}", e)))?
            // Increase data windows for better throughput (16MB connection, 8MB per stream)
            .with_data_window(16 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set data window: {:?}", e)))?
            .with_bidirectional_local_data_window(8 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set bidirectional window: {:?}", e)))?
            // Optimize for local network performance
            .with_initial_round_trip_time(Duration::from_millis(1)) // Low RTT for local connections
            .map_err(|e| RpcError::ConfigError(format!("Failed to set RTT: {:?}", e)))?
            .with_max_ack_delay(Duration::from_millis(5)) // Faster ACK responses
            .map_err(|e| RpcError::ConfigError(format!("Failed to set ACK delay: {:?}", e)))?
            // Increase send buffer for better performance
            .with_max_send_buffer_size(2 * 1024 * 1024) // 2MB send buffer
            .map_err(|e| RpcError::ConfigError(format!("Failed to set send buffer: {:?}", e)))?;

        let server = s2n_quic::Server::builder()
            .with_tls((self.config.cert_path.as_path(), key_path.as_path()))
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_limits(limits)
            .map_err(|e| RpcError::ConfigError(format!("Failed to apply limits: {:?}", e)))?
            .with_io(self.config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let local_addr = server.local_addr().map_err(|_err| {
            RpcError::ConfigError("Could not retrieve local_addr() from server".to_string())
        })?;

        self.socket_addr = Some(local_addr);
        println!("RPC server listening on {local_addr}");
        Ok(server)
    }
}

/// RPC client for making calls to remote servers over QUIC.
///
/// The client manages a QUIC connection to a server and provides methods for
/// making RPC calls. It handles connection management, request ID generation,
/// stream creation, and response processing automatically.
///
/// # Connection Management
///
/// Each client maintains a single QUIC connection to the server, but can make
/// multiple concurrent RPC calls over different streams within that connection.
/// This provides excellent performance for applications that make many requests
/// to the same server.
///
/// # Thread Safety
///
/// The client is designed to be used from multiple threads concurrently. All
/// methods are async and thread-safe, allowing you to make multiple RPC calls
/// simultaneously from different tasks.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,no_run
/// use rpcnet::{RpcClient, RpcConfig};
/// use std::net::SocketAddr;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Configure client
///     let config = RpcConfig::new("ca-cert.pem", "127.0.0.1:0")
///         .with_server_name("myserver.example.com");
///
///     // Connect to server
///     let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
///     let client = RpcClient::connect(server_addr, config).await?;
///
///     // Make an RPC call
///     let request_data = bincode::serialize(&"Hello, Server!")?;
///     let response = client.call("echo", request_data).await?;
///     let result: String = bincode::deserialize(&response)?;
///     
///     println!("Server replied: {}", result);
///     Ok(())
/// }
/// ```
///
/// ## Concurrent Calls
///
/// ```rust,no_run
/// use rpcnet::{RpcClient, RpcConfig};
/// use std::{net::SocketAddr, sync::Arc};
/// use tokio::join;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
///     let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
///     let client = Arc::new(RpcClient::connect(server_addr, config).await?);
///
///     // Make multiple concurrent calls
///     let client1 = client.clone();
///     let client2 = client.clone();
///
///     let (result1, result2) = join!(
///         client1.call("method1", vec![]),
///         client2.call("method2", vec![])
///     );
///
///     println!("Results: {:?}, {:?}", result1, result2);
///     Ok(())
/// }
/// ```
pub struct RpcClient {
    /// Shared QUIC connection to the server.
    /// 
    /// Protected by RwLock to allow concurrent access while supporting
    /// connection-level operations that need exclusive access.
    connection: Arc<RwLock<s2n_quic::Connection>>,
    
    /// Atomic counter for generating unique request IDs.
    /// 
    /// Each RPC call gets a unique ID that's used to match responses
    /// to requests, enabling correct handling of concurrent calls.
    pub next_id: Arc<AtomicU64>,
}

impl RpcClient {
    /// Establishes a connection to an RPC server.
    ///
    /// This method creates a new QUIC connection to the specified server address
    /// using the provided configuration. The connection includes TLS verification
    /// and optional keep-alive settings.
    ///
    /// # TLS Verification
    ///
    /// The client will verify the server's TLS certificate using the certificate
    /// specified in the config. The server name in the config must match the
    /// certificate's common name or subject alternative names.
    ///
    /// # Parameters
    ///
    /// - `connect_addr`: The server's socket address to connect to
    /// - `config`: Client configuration including TLS certificates and network settings
    ///
    /// # Returns
    ///
    /// Returns a connected client ready to make RPC calls, or an error if the
    /// connection could not be established.
    ///
    /// # Errors
    ///
    /// - [`RpcError::TlsError`] if TLS setup or certificate verification fails
    /// - [`RpcError::ConnectionError`] if the network connection fails
    /// - [`RpcError::ConfigError`] if the configuration is invalid
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use std::net::SocketAddr;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Connect to a server
    ///     let config = RpcConfig::new("server-cert.pem", "127.0.0.1:0")
    ///         .with_server_name("myserver.local");
    ///         
    ///     let server_addr: SocketAddr = "192.168.1.100:8080".parse()?;
    ///     let client = RpcClient::connect(server_addr, config).await?;
    ///     
    ///     println!("Connected successfully!");
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(connect_addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        // Create optimized limits matching server configuration
        let limits = s2n_quic::provider::limits::Limits::new()
            .with_max_open_local_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client stream limits: {:?}", e)))?
            .with_max_open_remote_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client stream limits: {:?}", e)))?
            .with_data_window(16 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client data window: {:?}", e)))?
            .with_bidirectional_local_data_window(8 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client bidirectional window: {:?}", e)))?
            .with_initial_round_trip_time(Duration::from_millis(1))
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client RTT: {:?}", e)))?
            .with_max_ack_delay(Duration::from_millis(5))
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client ACK delay: {:?}", e)))?
            .with_max_send_buffer_size(2 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client send buffer: {:?}", e)))?;

        let client = Client::builder()
            .with_tls(config.cert_path.as_path())
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_limits(limits)
            .map_err(|e| RpcError::ConfigError(format!("Failed to apply client limits: {:?}", e)))?
            .with_io(config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let connect = Connect::new(connect_addr).with_server_name(config.server_name.as_str());
        let mut connection = client
            .connect(connect)
            .await
            .map_err(|e| RpcError::ConnectionError(e.to_string()))?;

        if let Some(_interval) = config.keep_alive_interval {
            connection
                .keep_alive(true)
                .map_err(|e| RpcError::ConfigError(e.to_string()))?;
        }

        Ok(Self {
            connection: Arc::new(RwLock::new(connection)),
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Makes an RPC call to the server.
    ///
    /// This method sends an RPC request to the server and waits for a response.
    /// Each call uses a new bidirectional QUIC stream, allowing for excellent
    /// concurrency when making multiple calls. The call includes automatic
    /// timeout handling based on [`DEFAULT_TIMEOUT`].
    ///
    /// # Call Process
    ///
    /// 1. Generates a unique request ID for response matching
    /// 2. Creates a new bidirectional QUIC stream  
    /// 3. Serializes and sends the request
    /// 4. Reads and deserializes the response
    /// 5. Matches response to request by ID
    /// 6. Returns result data or propagates server errors
    ///
    /// # Parameters
    ///
    /// - `method`: The name of the RPC method to call on the server
    /// - `params`: Serialized parameters to send to the method handler
    ///
    /// # Returns
    ///
    /// Returns the serialized response data from the server on success.
    /// You'll typically want to deserialize this data into your expected response type.
    ///
    /// # Errors
    ///
    /// - [`RpcError::StreamError`] if stream operations fail or server returns an error
    /// - [`RpcError::ConnectionError`] if the connection is lost
    /// - [`RpcError::SerializationError`] if request serialization fails
    /// - [`RpcError::Timeout`] if the server doesn't respond within the timeout period
    ///
    /// # Examples
    ///
    /// ## Simple Call
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use std::net::SocketAddr;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    ///     let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    ///     let client = RpcClient::connect(addr, config).await?;
    ///
    ///     // Call a method with no parameters
    ///     let response = client.call("ping", vec![]).await?;
    ///     let result: String = bincode::deserialize(&response)?;
    ///     println!("Server said: {}", result);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Call with Parameters
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use serde::{Serialize, Deserialize};
    /// use std::net::SocketAddr;
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct MathRequest { a: i32, b: i32 }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    ///     let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    ///     let client = RpcClient::connect(addr, config).await?;
    ///
    ///     // Call a method with parameters
    ///     let request = MathRequest { a: 10, b: 5 };
    ///     let params = bincode::serialize(&request)?;
    ///     let response = client.call("add", params).await?;
    ///     
    ///     let sum: i32 = bincode::deserialize(&response)?;
    ///     println!("10 + 5 = {}", sum);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Error Handling
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig, RpcError};
    /// use std::net::SocketAddr;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    ///     let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    ///     let client = RpcClient::connect(addr, config).await?;
    ///
    ///     match client.call("risky_operation", vec![]).await {
    ///         Ok(response) => {
    ///             println!("Success: {:?}", response);
    ///         }
    ///         Err(RpcError::Timeout) => {
    ///             println!("Server took too long to respond");
    ///         }
    ///         Err(RpcError::StreamError(msg)) => {
    ///             println!("Server error: {}", msg);
    ///         }
    ///         Err(e) => {
    ///             println!("Other error: {}", e);
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn call(&self, method: &str, params: Vec<u8>) -> Result<Vec<u8>, RpcError> {
        // Generate a new request ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest::new(id, method.to_string(), params);
        // Pre-allocate serialization buffer to avoid reallocations
        let req_data = bincode::serialize(&req)?;

        // Open a new bidirectional stream with minimal lock time
        let mut stream = {
            let mut conn = self.connection.write().await;
            conn.open_bidirectional_stream()
                .await
                .map_err(|e| RpcError::StreamError(e.to_string()))?
        }; // Lock released immediately after stream creation

        // Send the request
        stream
            .send(req_data.into())
            .await
            .map_err(|e| RpcError::StreamError(e.to_string()))?;

        // Read back the response with optimized buffering
        let read_future = async {
            // Use BytesMut for more efficient buffer management
            let mut response_data = BytesMut::with_capacity(1024);
            while let Ok(Some(chunk)) = stream.receive().await {
                response_data.extend_from_slice(&chunk);
                
                // Only attempt deserialization if we have a reasonable amount of data
                if response_data.len() >= 16 { // Minimum for a valid response
                    if let Ok(response) = bincode::deserialize::<RpcResponse>(&response_data[..]) {
                        if response.id() == id {
                            // Extract data without cloning when possible
                            return match (response.result(), response.error()) {
                                (Some(data), None) => Ok(data.to_vec()), // More explicit about the copy
                                (None, Some(err_msg)) => Err(RpcError::StreamError(err_msg.to_string())), // Already owned
                                _ => Err(RpcError::StreamError("Invalid response".into())), // Avoid string allocation
                            };
                        }
                    }
                }
            }
            // If we exit the loop without returning, the stream closed early or never gave a valid response
            Err(RpcError::ConnectionError(
                "Stream closed unexpectedly".into(),
            ))
        };

        // Enforce the DEFAULT_TIMEOUT
        match tokio::time::timeout(DEFAULT_TIMEOUT, read_future).await {
            Ok(res) => res,
            Err(_) => Err(RpcError::Timeout),
        }
    }

    /// Calls a streaming RPC method where the client sends multiple requests and receives multiple responses.
    ///
    /// This method enables bidirectional streaming where both the client and server can send
    /// multiple messages over a single stream. This is useful for real-time communication,
    /// bulk operations, or scenarios where multiple request/response pairs need to be processed
    /// efficiently over a single connection.
    ///
    /// # Stream Protocol
    ///
    /// Each message in the stream is length-prefixed to enable proper message framing:
    /// - 4 bytes: Message length (little-endian u32)
    /// - N bytes: Serialized message data
    ///
    /// # Parameters
    ///
    /// - `method`: The name of the streaming RPC method to call
    /// - `request_stream`: A stream of request data (Vec<u8>) to send to the server
    ///
    /// # Returns
    ///
    /// Returns a stream of response data (Vec<u8>) from the server.
    ///
    /// # Errors
    ///
    /// - [`RpcError::StreamError`] if the stream encounters an error
    /// - [`RpcError::ConnectionError`] if the connection fails
    /// - [`RpcError::SerializationError`] if message framing fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use futures::{stream, StreamExt};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
    ///         .with_server_name("localhost");
    ///     let client = RpcClient::connect("127.0.0.1:8080".parse()?, config).await?;
    ///
    ///     // Create a stream of requests
    ///     let requests = stream::iter(vec![
    ///         b"request1".to_vec(),
    ///         b"request2".to_vec(),
    ///         b"request3".to_vec(),
    ///     ]);
    ///
    ///     // Call streaming method
    ///     let response_stream = client.call_streaming("echo_stream", requests).await?;
    ///     let mut response_stream = Box::pin(response_stream);
    ///
    ///     // Process responses
    ///     while let Some(response) = response_stream.next().await {
    ///         let response = response?;
    ///         println!("Received: {}", String::from_utf8_lossy(&response));
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn call_streaming<S>(
        &self,
        method: &str,
        request_stream: S,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, RpcError>>, RpcError>
    where
        S: Stream<Item = Vec<u8>> + Send + 'static,
    {
        // Open a new bidirectional stream
        let mut stream = {
            let mut conn = self.connection.write().await;
            conn.open_bidirectional_stream()
                .await
                .map_err(|e| RpcError::StreamError(e.to_string()))?
        };

        // Send the method name first (with length prefix)
        let method_data = method.as_bytes();
        let method_len = (method_data.len() as u32).to_le_bytes();
        stream
            .send([&method_len[..], method_data].concat().into())
            .await
            .map_err(|e| RpcError::StreamError(e.to_string()))?;
        
        // Removed artificial delay for better performance

        // Use Arc<tokio::sync::Mutex> to share the stream
        let stream = std::sync::Arc::new(tokio::sync::Mutex::new(stream));
        let send_stream = stream.clone();

        // Spawn a task to send requests
        let mut request_stream = Box::pin(request_stream);
        tokio::spawn(async move {
            let mut _count = 0;
            while let Some(request_data) = request_stream.next().await {
                _count += 1;
                let data_len = (request_data.len() as u32).to_le_bytes();
                let mut stream_guard = send_stream.lock().await;
                if let Err(_e) = stream_guard.send([&data_len[..], &request_data].concat().into()).await {
                    break;
                }
                drop(stream_guard); // Release lock
            }
            // Send empty frame to signal end of requests
            let mut stream_guard = send_stream.lock().await;
            let _ = stream_guard.send(vec![0, 0, 0, 0].into()).await;
        });

        let receive_stream = stream.clone();
        // Return a stream of responses
        Ok(async_stream::stream! {
            let mut buffer = BytesMut::with_capacity(8192);
            
            loop {
                let chunk = {
                    let mut stream_guard = receive_stream.lock().await;
                    stream_guard.receive().await
                };
                
                if let Ok(Some(chunk)) = chunk {
                    buffer.extend_from_slice(&chunk);
                    
                    // Try to parse complete messages
                    while buffer.len() >= 4 {
                        let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
                        
                        if len == 0 {
                            // End of stream marker
                            return;
                        }
                        
                        if buffer.len() >= 4 + len {
                            // We have a complete message
                            let message_data = buffer.split_to(4 + len);
                            let response_data = message_data[4..].to_vec();
                            yield Ok(response_data);
                        } else {
                            // Need more data
                            break;
                        }
                    }
                } else {
                    // Connection closed or error
                    break;
                }
            }
        })
    }

    /// Calls a server-streaming RPC method where the client sends one request and receives multiple responses.
    ///
    /// This method is useful for scenarios where a single request should generate multiple
    /// responses, such as database queries returning multiple rows, file listings, or
    /// real-time updates.
    ///
    /// # Parameters
    ///
    /// - `method`: The name of the server-streaming RPC method to call
    /// - `request`: The request data to send to the server
    ///
    /// # Returns
    ///
    /// Returns a stream of response data from the server.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use futures::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
    ///         .with_server_name("localhost");
    ///     let client = RpcClient::connect("127.0.0.1:8080".parse()?, config).await?;
    ///
    ///     // Call server-streaming method
    ///     let response_stream = client.call_server_streaming("list_files", b"/home".to_vec()).await?;
    ///     let mut response_stream = Box::pin(response_stream);
    ///
    ///     // Process responses
    ///     while let Some(response) = response_stream.next().await {
    ///         let file_info = response?;
    ///         println!("File: {}", String::from_utf8_lossy(&file_info));
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn call_server_streaming(
        &self,
        method: &str,
        request: Vec<u8>,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, RpcError>>, RpcError> {
        use futures::stream;
        
        // Create a single-item stream for the request
        let request_stream = stream::iter(vec![request]);
        
        // Use the bidirectional streaming method
        self.call_streaming(method, request_stream).await
    }

    /// Calls a client-streaming RPC method where the client sends multiple requests and receives one response.
    ///
    /// This method is useful for scenarios where multiple related requests should be processed
    /// together and return a single aggregated response, such as bulk inserts, file uploads,
    /// or batch processing operations.
    ///
    /// # Parameters
    ///
    /// - `method`: The name of the client-streaming RPC method to call
    /// - `request_stream`: A stream of request data to send to the server
    ///
    /// # Returns
    ///
    /// Returns a single aggregated response from the server.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rpcnet::{RpcClient, RpcConfig};
    /// use futures::stream;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
    ///         .with_server_name("localhost");
    ///     let client = RpcClient::connect("127.0.0.1:8080".parse()?, config).await?;
    ///
    ///     // Create a stream of data to upload
    ///     let data_chunks = stream::iter(vec![
    ///         b"chunk1".to_vec(),
    ///         b"chunk2".to_vec(),
    ///         b"chunk3".to_vec(),
    ///     ]);
    ///
    ///     // Call client-streaming method
    ///     let result = client.call_client_streaming("upload_file", data_chunks).await?;
    ///     println!("Upload result: {}", String::from_utf8_lossy(&result));
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn call_client_streaming<S>(
        &self,
        method: &str,
        request_stream: S,
    ) -> Result<Vec<u8>, RpcError>
    where
        S: Stream<Item = Vec<u8>> + Send + 'static,
    {
        // Use the bidirectional streaming method and collect the first response
        let response_stream = self.call_streaming(method, request_stream).await?;
        let mut response_stream = Box::pin(response_stream);
        
        match response_stream.next().await {
            Some(Ok(response)) => Ok(response),
            Some(Err(e)) => Err(e),
            None => Err(RpcError::StreamError("No response received".to_string())),
        }
    }
}

// ==========================
//          TESTS
// ==========================
#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::SocketAddr, str::FromStr};
    use tokio::{spawn, time::sleep};

    // For your local tests, adjust cert/key paths if needed:
    fn test_config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(30))
    }

    /// Create a real QUIC server on ephemeral port; returns (addr, join_handle)
    async fn start_test_server(
        maybe_server: Option<RpcServer>,
    ) -> Result<(SocketAddr, tokio::task::JoinHandle<Result<(), RpcError>>), RpcError> {
        let server = if let Some(s) = maybe_server {
            s
        } else {
            let s = RpcServer::new(test_config());
            // A simple "echo" handler
            s.register("echo", |params| async move {
                Ok(params) // just echo
            })
            .await;
            s
        };

        let key_path = server
            .config
            .key_path
            .as_ref()
            .ok_or_else(|| RpcError::ConfigError("No key path".into()))?;

        let mut quic_server = s2n_quic::Server::builder()
            .with_tls((server.config.cert_path.as_path(), key_path.as_path()))
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_io(server.config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let local_addr = quic_server
            .local_addr()
            .map_err(|_| RpcError::ConfigError("Could not retrieve local addr".into()))?;

        let handlers = server.handlers.clone();
        let handle = spawn(async move {
            while let Some(mut connection) = quic_server.accept().await {
                let handlers = handlers.clone();
                tokio::spawn(async move {
                    while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await
                    {
                        let handlers = handlers.clone();
                        tokio::spawn(async move {
                            let mut request_data = Vec::with_capacity(8192);
                            while let Ok(Some(data)) = stream.receive().await {
                                request_data.extend_from_slice(&data);
                                if let Ok(request) =
                                    bincode::deserialize::<RpcRequest>(&request_data)
                                {
                                    let handlers = handlers.read().await;
                                    let response = match handlers.get(request.method()) {
                                        Some(handler) => {
                                            let result = handler(request.params().to_vec()).await;
                                            RpcResponse::from_result(request.id(), result)
                                        }
                                        None => RpcResponse::new(
                                            request.id(),
                                            None,
                                            Some(format!("Unknown method: {}", request.method())),
                                        ),
                                    };
                                    if let Ok(resp_data) = bincode::serialize(&response) {
                                        let _ = stream.send(resp_data.into()).await;
                                    }
                                    break;
                                }
                            }
                        });
                    }
                });
            }
            Ok(())
        });

        Ok((local_addr, handle))
    }

    // -----------------------------------
    // tests
    // -----------------------------------
    #[tokio::test]
    async fn test_config_builder() {
        let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080")
            .with_key_path("certs/key.pem")
            .with_server_name("mytest.server")
            .with_keep_alive_interval(Duration::from_secs(60));

        assert_eq!(config.cert_path, PathBuf::from("certs/cert.pem"));
        assert_eq!(config.key_path, Some(PathBuf::from("certs/key.pem")));
        assert_eq!(config.server_name, "mytest.server");
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_register_handler() {
        let server = RpcServer::new(test_config());
        server
            .register("test", |params| async move {
                Ok(params) // echo
            })
            .await;

        let handlers = server.handlers.read().await;
        assert!(handlers.contains_key("test"));
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let server = RpcServer::new(test_config());
        // no method registered => unknown

        // We'll do a small direct test function:
        async fn handle_request_direct(
            server: &RpcServer,
            req_data: Vec<u8>,
        ) -> Result<Vec<u8>, RpcError> {
            let req: RpcRequest = bincode::deserialize(&req_data)?;
            let handlers = server.handlers.read().await;
            let h = handlers
                .get(req.method())
                .ok_or_else(|| RpcError::UnknownMethod(req.method().to_string()))?;

            let result = h(req.params().to_vec()).await;
            let resp = RpcResponse::from_result(req.id(), result);
            Ok(bincode::serialize(&resp)?)
        }

        let req = RpcRequest::new(1, "unknown".into(), vec![]);
        let data = bincode::serialize(&req).unwrap();

        let res = handle_request_direct(&server, data).await;
        match res {
            Err(RpcError::UnknownMethod(m)) => assert_eq!(m, "unknown"),
            other => panic!("Expected UnknownMethod, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_handle_request() {
        let server = RpcServer::new(test_config());
        server
            .register("echo", |params| async move {
                Ok(params) // echo
            })
            .await;

        async fn handle_request_direct(
            server: &RpcServer,
            req_data: Vec<u8>,
        ) -> Result<Vec<u8>, RpcError> {
            let req: RpcRequest = bincode::deserialize(&req_data)?;
            let handlers = server.handlers.read().await;
            let h = handlers
                .get(req.method())
                .ok_or_else(|| RpcError::UnknownMethod(req.method().to_string()))?;

            let result = h(req.params().to_vec()).await;
            let resp = RpcResponse::from_result(req.id(), result);
            Ok(bincode::serialize(&resp)?)
        }

        let req = RpcRequest::new(42, "echo".into(), b"hello".to_vec());
        let data = bincode::serialize(&req).unwrap();
        let res_data = handle_request_direct(&server, data).await.unwrap();
        let resp: RpcResponse = bincode::deserialize(&res_data).unwrap();

        assert_eq!(resp.id(), 42);
        assert_eq!(resp.result().unwrap(), b"hello");
    }

    #[tokio::test]
    async fn test_client_connection() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        assert_eq!(client.next_id.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_call_timeout() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());
        server
            .register("slow_method", |_params| async {
                // Sleep 3s so the 2s test timeout reliably hits
                sleep(Duration::from_secs(3)).await;
                Ok(b"done".to_vec())
            })
            .await;

        let (addr, _jh) = start_test_server(Some(server)).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        // Expect a 2s timeout
        let result = client.call("slow_method", vec![]).await;
        assert!(matches!(result, Err(RpcError::Timeout)));

        Ok(())
    }

    #[tokio::test]
    async fn test_request_ids() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        let id1 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id3 = client.next_id.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_calls() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());
        server
            .register("test_method", |_params| async {
                // Sleep longer than the 2s default test timeout
                sleep(Duration::from_secs(3)).await;
                Ok(vec![1, 2, 3])
            })
            .await;

        let (addr, _jh) = start_test_server(Some(server)).await?;
        let client = Arc::new(RpcClient::connect(addr, test_config()).await?);

        let mut tasks = vec![];
        for _ in 0..5 {
            let c = client.clone();
            tasks.push(tokio::spawn(
                async move { c.call("test_method", vec![]).await },
            ));
        }

        for t in tasks {
            let res = t.await.unwrap();
            assert!(matches!(res, Err(RpcError::Timeout)));
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_error() -> Result<(), RpcError> {
        // Connect to something that doesn't exist
        let addr = SocketAddr::from_str("127.0.0.1:9999").unwrap();
        let res = RpcClient::connect(addr, test_config()).await;
        assert!(matches!(res, Err(RpcError::ConnectionError(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_server_bind_error_missing_key() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0");
        // No key path configured
        let mut server = RpcServer::new(config);
        let result = server.bind();
        assert!(matches!(result, Err(RpcError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_server_socket_addr() -> Result<(), RpcError> {
        let mut server = RpcServer::new(test_config());
        assert_eq!(server.socket_addr, None);

        let _quic_server = server.bind()?;
        assert!(server.socket_addr.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_rpc_request_accessors() {
        let params = vec![1, 2, 3, 4, 5];
        let request = RpcRequest::new(42, "test_method".to_string(), params.clone());

        assert_eq!(request.id(), 42);
        assert_eq!(request.method(), "test_method");
        assert_eq!(request.params(), &params);
    }

    #[tokio::test]
    async fn test_rpc_response_accessors() {
        let result_data = vec![10, 20, 30];
        let error_msg = "test error".to_string();

        // Test success response
        let success_response = RpcResponse::new(123, Some(result_data.clone()), None);
        assert_eq!(success_response.id(), 123);
        assert_eq!(success_response.result(), Some(&result_data));
        assert_eq!(success_response.error(), None);

        // Test error response
        let error_response = RpcResponse::new(456, None, Some(error_msg.clone()));
        assert_eq!(error_response.id(), 456);
        assert_eq!(error_response.result(), None);
        assert_eq!(error_response.error(), Some(&error_msg));
    }

    #[tokio::test]
    async fn test_rpc_response_from_result() {
        // Test Ok result
        let ok_result: Result<Vec<u8>, RpcError> = Ok(vec![1, 2, 3]);
        let response = RpcResponse::from_result(100, ok_result);
        assert_eq!(response.id(), 100);
        assert_eq!(response.result(), Some(&vec![1, 2, 3]));
        assert_eq!(response.error(), None);

        // Test Err result
        let err_result: Result<Vec<u8>, RpcError> = Err(RpcError::Timeout);
        let response = RpcResponse::from_result(200, err_result);
        assert_eq!(response.id(), 200);
        assert_eq!(response.result(), None);
        assert!(response.error().is_some());
        assert!(response.error().unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn test_config_cloning() {
        let original = RpcConfig::new("test.pem", "127.0.0.1:8080")
            .with_key_path("key.pem")
            .with_server_name("test.server")
            .with_keep_alive_interval(Duration::from_secs(60));

        let cloned = original.clone();

        assert_eq!(original.cert_path, cloned.cert_path);
        assert_eq!(original.key_path, cloned.key_path);
        assert_eq!(original.bind_address, cloned.bind_address);
        assert_eq!(original.server_name, cloned.server_name);
        assert_eq!(original.keep_alive_interval, cloned.keep_alive_interval);
    }

    #[tokio::test]
    async fn test_error_display_formats() {
        let errors = vec![
            RpcError::ConnectionError("connection failed".to_string()),
            RpcError::StreamError("stream closed".to_string()),
            RpcError::TlsError("handshake failed".to_string()),
            RpcError::Timeout,
            RpcError::UnknownMethod("missing_method".to_string()),
            RpcError::ConfigError("bad config".to_string()),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
            println!("Error: {}", error_string);
        }
    }

    #[tokio::test]
    async fn test_large_payload_serialization() {
        let large_data = vec![0xAA; 100_000]; // 100KB
        let request = RpcRequest::new(999, "large_test".to_string(), large_data.clone());

        // Should serialize and deserialize successfully
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.id(), 999);
        assert_eq!(deserialized.method(), "large_test");
        assert_eq!(deserialized.params(), &large_data);
    }

    #[tokio::test]
    async fn test_empty_method_and_params() {
        let request = RpcRequest::new(0, "".to_string(), vec![]);
        assert_eq!(request.method(), "");
        assert!(request.params().is_empty());

        // Should be serializable
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.method(), "");
        assert!(deserialized.params().is_empty());
    }

    #[tokio::test]
    async fn test_multiple_handler_registration() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());

        // Register multiple handlers
        server
            .register("method1", |_| async move { Ok(b"response1".to_vec()) })
            .await;
        server
            .register("method2", |_| async move { Ok(b"response2".to_vec()) })
            .await;
        server
            .register("method3", |_| async move { Ok(b"response3".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 3);
        assert!(handlers.contains_key("method1"));
        assert!(handlers.contains_key("method2"));
        assert!(handlers.contains_key("method3"));

        Ok(())
    }

    #[tokio::test]
    async fn test_handler_overwrite() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());

        // Register handler
        server
            .register("test", |_| async move { Ok(b"first".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 1);
        drop(handlers);

        // Overwrite with new handler
        server
            .register("test", |_| async move { Ok(b"second".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 1); // Still only one handler
        assert!(handlers.contains_key("test"));

        Ok(())
    }

    #[tokio::test]
    async fn test_client_id_generation() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        // Initial ID should be 1
        assert_eq!(client.next_id.load(Ordering::SeqCst), 1);

        // After making a call, ID should be incremented
        let _response = client.call("echo", vec![1, 2, 3]).await?;
        assert_eq!(client.next_id.load(Ordering::SeqCst), 2);

        // Multiple calls should increment ID
        let _response = client.call("echo", vec![]).await?;
        let _response = client.call("echo", vec![]).await?;
        assert_eq!(client.next_id.load(Ordering::SeqCst), 4);

        Ok(())
    }

    #[tokio::test]
    async fn test_default_timeout_constant() {
        // Test that the timeout constant is properly defined
        #[cfg(test)]
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(2));

        #[cfg(not(test))]
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_config_pathbuf_compatibility() {
        use std::path::PathBuf;

        // Test with &str
        let config1 = RpcConfig::new("cert.pem", "127.0.0.1:0");
        assert_eq!(config1.cert_path, PathBuf::from("cert.pem"));

        // Test with String
        let config2 = RpcConfig::new("cert.pem".to_string(), "127.0.0.1:0".to_string());
        assert_eq!(config2.cert_path, PathBuf::from("cert.pem"));

        // Test with PathBuf
        let config3 = RpcConfig::new(PathBuf::from("cert.pem"), "127.0.0.1:0");
        assert_eq!(config3.cert_path, PathBuf::from("cert.pem"));
    }

    /// Test that the RpcConfig builder pattern works correctly.
    ///
    /// ```rust
    /// use rpcnet::RpcConfig;
    /// use std::time::Duration;
    /// use std::path::PathBuf;
    ///
    /// let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
    ///     .with_key_path("key.pem")
    ///     .with_server_name("example.com")
    ///     .with_keep_alive_interval(Duration::from_secs(60));
    ///
    /// assert_eq!(config.cert_path, PathBuf::from("cert.pem"));
    /// assert_eq!(config.bind_address, "127.0.0.1:8080");
    /// assert_eq!(config.server_name, "example.com");
    /// ```
    #[test]
    fn test_config_builder_doctest() {
        use std::time::Duration;
        use std::path::PathBuf;
        
        let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
            .with_key_path("key.pem")
            .with_server_name("example.com")
            .with_keep_alive_interval(Duration::from_secs(60));

        assert_eq!(config.cert_path, PathBuf::from("cert.pem"));
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.server_name, "example.com");
    }

    /// Test that RpcRequest can be created and accessed correctly.
    ///
    /// ```rust
    /// use rpcnet::RpcRequest;
    ///
    /// let request = RpcRequest::new(123, "test_method".to_string(), vec![1, 2, 3]);
    ///
    /// assert_eq!(request.id(), 123);
    /// assert_eq!(request.method(), "test_method");
    /// assert_eq!(request.params(), &[1, 2, 3]);
    /// ```
    #[test]
    fn test_request_creation_doctest() {
        let request = RpcRequest::new(123, "test_method".to_string(), vec![1, 2, 3]);

        assert_eq!(request.id(), 123);
        assert_eq!(request.method(), "test_method");
        assert_eq!(request.params(), &[1, 2, 3]);
    }

    /// Test that RpcResponse can be created from both success and error cases.
    ///
    /// ```rust
    /// use rpcnet::{RpcResponse, RpcError};
    ///
    /// // Success response
    /// let success = RpcResponse::new(1, Some(vec![42]), None);
    /// assert_eq!(success.id(), 1);
    /// assert_eq!(success.result(), Some(&vec![42]));
    /// assert!(success.error().is_none());
    ///
    /// // Error response
    /// let error = RpcResponse::new(2, None, Some("Error occurred".to_string()));
    /// assert_eq!(error.id(), 2);
    /// assert!(error.result().is_none());
    /// assert_eq!(error.error(), Some(&"Error occurred".to_string()));
    /// ```
    #[test]
    fn test_response_creation_doctest() {
        // Success response
        let success = RpcResponse::new(1, Some(vec![42]), None);
        assert_eq!(success.id(), 1);
        assert_eq!(success.result(), Some(&vec![42]));
        assert!(success.error().is_none());

        // Error response
        let error_resp = RpcResponse::new(2, None, Some("Error occurred".to_string()));
        assert_eq!(error_resp.id(), 2);
        assert!(error_resp.result().is_none());
        assert_eq!(error_resp.error(), Some(&"Error occurred".to_string()));
    }

    /// Test RpcError display formatting.
    ///
    /// ```rust
    /// use rpcnet::RpcError;
    ///
    /// let errors = vec![
    ///     RpcError::ConnectionError("failed".to_string()),
    ///     RpcError::StreamError("closed".to_string()),
    ///     RpcError::Timeout,
    /// ];
    ///
    /// for error in errors {
    ///     let display = error.to_string();
    ///     assert!(!display.is_empty());
    /// }
    /// ```
    #[test]
    fn test_error_display_doctest() {
        let errors = vec![
            RpcError::ConnectionError("failed".to_string()),
            RpcError::StreamError("closed".to_string()),
            RpcError::Timeout,
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
        }
    }

    /// Test serialization of RPC types.
    ///
    /// ```rust
    /// use rpcnet::{RpcRequest, RpcResponse};
    ///
    /// let request = RpcRequest::new(1, "test".to_string(), vec![1, 2, 3]);
    /// let serialized = bincode::serialize(&request).unwrap();
    /// let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
    ///
    /// assert_eq!(request.id(), deserialized.id());
    /// assert_eq!(request.method(), deserialized.method());
    /// assert_eq!(request.params(), deserialized.params());
    /// ```
    #[test]
    fn test_serialization_doctest() {
        let request = RpcRequest::new(1, "test".to_string(), vec![1, 2, 3]);
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(request.id(), deserialized.id());
        assert_eq!(request.method(), deserialized.method());
        assert_eq!(request.params(), deserialized.params());
    }
}
