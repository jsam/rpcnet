//! # Code Generation for RpcNet Services
//!
//! This module provides automatic code generation capabilities for RpcNet services.
//! It transforms service definitions written in Rust syntax into type-safe client
//! and server implementations.
//!
//! ## Overview
//!
//! The code generator takes `.rpc.rs` files containing service definitions and generates:
//! - **Types**: Request/response structs and error enums
//! - **Server**: Handler trait and server implementation
//! - **Client**: Type-safe client with method stubs
//!
//! ## Code Generation Workflow
//!
//! 1. **Parse**: Service definitions using the `syn` crate
//! 2. **Validate**: Ensure all methods are async and return `Result<T, E>`
//! 3. **Generate**: Client and server code using the `quote` crate
//! 4. **Format**: Generated code using `prettyplease`
//!
//! ## Service Definition Format
//!
//! Service definitions use standard Rust syntax with the `#[rpc_trait]` attribute:
//!
//! ```rust,ignore
//! use serde::{Serialize, Deserialize};
//! use rpcnet::prelude::*;
//!
//! // Request/response types
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct MyRequest {
//!     pub field: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub struct MyResponse {
//!     pub result: String,
//! }
//!
//! // Service-specific errors
//! #[derive(Serialize, Deserialize, Debug, Clone)]
//! pub enum MyError {
//!     InvalidInput,
//!     ServiceUnavailable,
//! }
//!
//! // Service trait definition
//! #[rpc_trait]
//! pub trait MyService {
//!     async fn my_method(&self, request: MyRequest) -> Result<MyResponse, MyError>;
//! }
//! ```
//!
//! ## Generated Code Structure
//!
//! For a service named `MyService`, the generator creates:
//!
//! ```text
//! generated/myservice/
//! ├── mod.rs          # Module exports and re-exports
//! ├── types.rs        # Request, response, and error types
//! ├── server.rs       # MyServiceHandler trait and MyServiceServer struct
//! └── client.rs       # MyServiceClient struct with typed methods
//! ```
//!
//! ### Server Code
//!
//! The generated server code includes:
//! - **Handler Trait**: You implement this trait with your business logic
//! - **Server Struct**: Manages RPC registration and routing
//! - **Registration**: Automatic method registration with proper serialization
//!
//! ```rust,ignore
//! // Generated handler trait
//! #[async_trait]
//! pub trait MyServiceHandler: Send + Sync + 'static {
//!     async fn my_method(&self, request: MyRequest) -> Result<MyResponse, MyError>;
//! }
//!
//! // Generated server struct
//! pub struct MyServiceServer<H: MyServiceHandler> {
//!     handler: Arc<H>,
//!     rpc_server: RpcServer,
//! }
//! ```
//!
//! ### Client Code
//!
//! The generated client code provides:
//! - **Type-safe Methods**: Each service method becomes a typed client method
//! - **Automatic Serialization**: Request/response serialization handled automatically
//! - **Error Mapping**: Service errors are properly typed
//!
//! ```rust,ignore
//! // Generated client struct
//! pub struct MyServiceClient {
//!     inner: RpcClient,
//! }
//!
//! impl MyServiceClient {
//!     pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> { ... }
//!     pub async fn my_method(&self, request: MyRequest) -> Result<MyResponse, RpcError> { ... }
//! }
//! ```
//!
//! ## Usage Examples
//!
//! ### CLI Tool
//!
//! ```bash
//! # Generate code from a service definition
//! rpcnet-gen --input service.rpc.rs --output generated
//!
//! # Generate only server code
//! rpcnet-gen --input service.rpc.rs --output generated --server-only
//! ```
//!
//! ### Build Script Integration
//!
//! ```rust,no_run
//! // In build.rs
//! println!("cargo:rerun-if-changed=service.rpc.rs");
//!
//! rpcnet::codegen::Builder::new()
//!     .input("service.rpc.rs")
//!     .output("src/generated")
//!     .build()
//!     .expect("Failed to generate RPC code");
//! ```
//!
//! ### Multiple Services
//!
//! ```rust,no_run
//! // In build.rs for multiple services
//! let services = ["user.rpc.rs", "auth.rpc.rs", "data.rpc.rs"];
//!
//! for service in &services {
//!     println!("cargo:rerun-if-changed={}", service);
//!
//!     rpcnet::codegen::Builder::new()
//!         .input(service)
//!         .output("src/generated")
//!         .build()
//!         .expect("Failed to generate RPC code");
//! }
//! ```
//!
//! ## Advanced Features
//!
//! ### Service Validation
//!
//! The code generator performs several validations:
//! - All service methods must be `async`
//! - All methods must have `&self` as the first parameter
//! - All methods must return `Result<T, E>`
//! - Service traits must have the `#[rpc_trait]` attribute
//!
//! ### Error Handling
//!
//! Generated code handles two types of errors:
//! - **Transport Errors**: Network, serialization, and protocol errors (RpcError)
//! - **Service Errors**: Your domain-specific errors defined in the service
//!
//! Client methods return `Result<ResponseType, RpcError>`, where service errors
//! are wrapped inside RpcError::StreamError.
//!
//! ### Performance Considerations
//!
//! - Generated code uses efficient binary serialization (bincode)
//! - Connection reuse is handled automatically
//! - Method calls are properly typed at compile time
//! - No runtime reflection or dynamic dispatch

#[cfg(feature = "codegen")]
mod generator;
#[cfg(feature = "codegen")]
mod parser;

#[cfg(feature = "codegen")]
pub use generator::CodeGenerator;
#[cfg(feature = "codegen")]
pub use parser::{ServiceDefinition, ServiceType};

use std::path::{Path, PathBuf};

/// Builder API for use in build scripts.
///
/// This provides a convenient way to generate code from build.rs files.
///
/// # Example
///
/// ```rust,no_run
/// // In build.rs
/// rpcnet::codegen::Builder::new()
///     .input("rpc/calculator.rpc.rs")
///     .output("src/generated")
///     .build()
///     .expect("Failed to generate RPC code");
/// ```
#[cfg(feature = "codegen")]
pub struct Builder {
    inputs: Vec<PathBuf>,
    output: PathBuf,
}

#[cfg(feature = "codegen")]
impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "codegen")]
impl Builder {
    /// Creates a new code generation builder.
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            output: PathBuf::from("src/generated"),
        }
    }

    /// Adds an input .rpc.rs file to process.
    pub fn input<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.inputs.push(path.as_ref().to_path_buf());
        self
    }

    /// Sets the output directory for generated code.
    pub fn output<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.output = path.as_ref().to_path_buf();
        self
    }

    /// Generates code for all input files.
    pub fn build(self) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;

        // Create output directory
        fs::create_dir_all(&self.output)?;

        for input in &self.inputs {
            // Read input file
            let content = fs::read_to_string(input)?;

            // Parse service definition
            let definition = ServiceDefinition::parse(&content)?;

            // Generate code
            let generator = CodeGenerator::new(definition);

            // Generate server and client code
            let server_code = generator.generate_server();
            let client_code = generator.generate_client();
            let types_code = generator.generate_types();

            // Format code
            let server_formatted = format_code(server_code)?;
            let client_formatted = format_code(client_code)?;
            let types_formatted = format_code(types_code)?;

            // Determine output subdirectory based on input filename
            // For files like "calculator.rpc.rs", extract "calculator"
            let service_name = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("generated");
            let service_name = if let Some(stripped) = service_name.strip_suffix(".rpc") {
                stripped // Remove ".rpc" suffix
            } else {
                service_name
            };
            let service_dir = self.output.join(service_name);
            fs::create_dir_all(&service_dir)?;

            // Write files
            fs::write(service_dir.join("server.rs"), server_formatted)?;
            fs::write(service_dir.join("client.rs"), client_formatted)?;
            fs::write(service_dir.join("types.rs"), types_formatted)?;

            // Generate mod.rs
            let mod_content = format!(
                r#"//! Generated code for {} service.

pub mod types;
pub mod server;
pub mod client;

pub use types::*;
"#,
                service_name
            );
            fs::write(service_dir.join("mod.rs"), mod_content)?;
        }

        Ok(())
    }
}

#[cfg(feature = "codegen")]
fn format_code(tokens: proc_macro2::TokenStream) -> Result<String, Box<dyn std::error::Error>> {
    let file = syn::parse2::<syn::File>(tokens)?;
    Ok(prettyplease::unparse(&file))
}

#[cfg(all(test, feature = "codegen"))]
mod tests {
    use super::*;
    use quote::quote;
    use std::fs;
    use tempfile::TempDir;

    fn sample_service() -> &'static str {
        r#"
            use rpcnet::prelude::*;

            #[rpc_trait]
            pub trait Example {
                async fn run(&self, request: ExampleRequest) -> Result<ExampleResponse, ExampleError>;
            }

            pub struct ExampleRequest;
            pub struct ExampleResponse;
            pub enum ExampleError { Boom }
        "#
    }

    #[test]
    fn builder_accumulates_inputs_and_output() {
        let builder = Builder::new()
            .input("first.rpc.rs")
            .input("second.rpc.rs")
            .output("custom/out");

        assert_eq!(builder.inputs.len(), 2);
        assert!(builder.inputs[0].ends_with("first.rpc.rs"));
        assert!(builder.inputs[1].ends_with("second.rpc.rs"));
        assert!(builder.output.ends_with("custom/out"));
    }

    #[test]
    fn builder_generates_directory_for_plain_filenames() {
        let temp = TempDir::new().expect("temp dir");
        let input = temp.path().join("analytics.rs");
        let output = temp.path().join("generated");

        fs::write(&input, sample_service()).expect("write service");

        Builder::new()
            .input(&input)
            .output(&output)
            .build()
            .expect("build should succeed");

        let service_dir = output.join("analytics");
        assert!(service_dir.join("server.rs").exists());
        assert!(service_dir.join("client.rs").exists());
        assert!(service_dir.join("types.rs").exists());
    }

    #[test]
    fn format_code_produces_pretty_source() {
        let tokens = quote! {
            fn main() {
                println!("hello");
            }
        };

        let formatted = format_code(tokens).expect("formatting");
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("println!"));
        // prettyplease adds newline at end of file
        assert!(formatted.ends_with("\n"));
    }
}
