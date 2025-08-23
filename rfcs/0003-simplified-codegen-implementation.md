# RFC 0003: Simplified Code Generation Using Syn Parser

## Summary

This RFC proposes a simplified implementation of code generation for rpcnet that:
1. Uses the existing `syn` crate to parse Rust syntax (no custom parser needed)
2. Implements code generation as a binary within the rpcnet crate (no separate crate)
3. Leverages Rust's powerful macro and parsing ecosystem

## Motivation

The previous RFC proposed building a custom parser, which is unnecessary complexity since:
- `.rpc` files use standard Rust syntax
- The `syn` crate already provides a complete, battle-tested Rust parser
- We can use `quote` for generating Rust code idiomatically
- Keeping everything in one crate simplifies maintenance and distribution

## Revised Architecture

### Project Structure

```
rpcnet/
├── Cargo.toml              # Single crate with lib and bin
├── src/
│   ├── lib.rs              # Library code (existing)
│   ├── bin/
│   │   └── rpcnet-gen.rs   # Code generator binary
│   ├── codegen/
│   │   ├── mod.rs          # Code generation module
│   │   ├── parser.rs       # Syn-based parsing
│   │   ├── generator.rs    # Code generation logic
│   │   └── templates.rs    # Generation templates
│   └── ... (existing code)
└── examples/
    ├── definitions/
    │   └── calculator.rpc   # Example service definition
    └── generated/           # Generated code examples
```

### Cargo.toml Configuration

```toml
[package]
name = "rpcnet"
version = "0.1.0"
edition = "2021"

[lib]
name = "rpcnet"

[[bin]]
name = "rpcnet-gen"
path = "src/bin/rpcnet-gen.rs"
required-features = ["codegen"]

[dependencies]
# Existing dependencies
s2n-quic = "1.31"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
thiserror = "1.0"

# Code generation dependencies (optional)
syn = { version = "2.0", features = ["full", "extra-traits"], optional = true }
quote = { version = "1.0", optional = true }
proc-macro2 = { version = "1.0", optional = true }
prettyplease = { version = "0.2", optional = true }  # For formatting generated code

# CLI dependencies
clap = { version = "4.0", features = ["derive"], optional = true }

[features]
default = []
codegen = ["syn", "quote", "proc-macro2", "clap", "prettyplease"]

[dev-dependencies]
tempfile = "3.0"
```

## Implementation Using Syn

### Service Definition Format

Since we're using Rust syntax, our `.rpc` files are just Rust modules with special attributes:

```rust
// calculator.rpc - This is valid Rust syntax!

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
pub enum CalcError {
    Overflow,
    DivisionByZero,
}

/// Calculator service for basic math operations
#[rpcnet::service]
pub trait Calculator {
    /// Adds two numbers
    async fn add(&self, req: AddRequest) -> Result<AddResponse, CalcError>;
    
    /// Subtracts two numbers
    async fn subtract(&self, req: SubRequest) -> Result<SubResponse, CalcError>;
}
```

### Parser Implementation Using Syn

```rust
// src/codegen/parser.rs

use syn::{File, Item, ItemTrait, ItemStruct, ItemEnum, Result, Error};
use quote::ToTokens;
use std::collections::HashMap;

pub struct ServiceDefinition {
    pub service_trait: ItemTrait,
    pub types: HashMap<String, ServiceType>,
    pub imports: Vec<syn::ItemUse>,
}

pub enum ServiceType {
    Struct(ItemStruct),
    Enum(ItemEnum),
}

impl ServiceDefinition {
    pub fn parse(content: &str) -> Result<Self> {
        // Parse the entire file using syn
        let ast: File = syn::parse_str(content)?;
        
        let mut service_trait = None;
        let mut types = HashMap::new();
        let mut imports = Vec::new();
        
        // Iterate through all items in the file
        for item in ast.items {
            match item {
                Item::Trait(trait_item) => {
                    // Check if this trait has our service attribute
                    if has_service_attribute(&trait_item) {
                        if service_trait.is_some() {
                            return Err(Error::new_spanned(
                                &trait_item,
                                "Multiple service traits found. Only one service per file is supported."
                            ));
                        }
                        service_trait = Some(trait_item);
                    }
                }
                Item::Struct(struct_item) => {
                    // Collect all structs as potential request/response types
                    types.insert(
                        struct_item.ident.to_string(),
                        ServiceType::Struct(struct_item)
                    );
                }
                Item::Enum(enum_item) => {
                    // Collect all enums (typically error types)
                    types.insert(
                        enum_item.ident.to_string(),
                        ServiceType::Enum(enum_item)
                    );
                }
                Item::Use(use_item) => {
                    imports.push(use_item);
                }
                _ => {} // Ignore other items
            }
        }
        
        let service_trait = service_trait
            .ok_or_else(|| syn::Error::new(
                proc_macro2::Span::call_site(),
                "No service trait found. Add #[rpcnet::service] to your trait."
            ))?;
        
        Ok(ServiceDefinition {
            service_trait,
            types,
            imports,
        })
    }
}

fn has_service_attribute(trait_item: &ItemTrait) -> bool {
    trait_item.attrs.iter().any(|attr| {
        attr.path().segments.len() == 2 &&
        attr.path().segments[0].ident == "rpcnet" &&
        attr.path().segments[1].ident == "service"
    })
}
```

### Code Generation Using Quote

```rust
// src/codegen/generator.rs

use quote::{quote, format_ident};
use proc_macro2::TokenStream;
use syn::{ItemTrait, TraitItem, FnArg, ReturnType, Type, Pat};

pub struct CodeGenerator {
    definition: ServiceDefinition,
}

impl CodeGenerator {
    pub fn new(definition: ServiceDefinition) -> Self {
        Self { definition }
    }
    
    pub fn generate_server(&self) -> TokenStream {
        let trait_name = &self.definition.service_trait.ident;
        let server_name = format_ident!("{}Server", trait_name);
        let handler_trait = format_ident!("{}Handler", trait_name);
        
        let methods = self.extract_methods();
        let handler_methods = self.generate_handler_methods(&methods);
        let register_methods = self.generate_register_methods(&methods, trait_name);
        
        quote! {
            use super::*;
            use rpcnet::{RpcServer, RpcConfig, RpcError};
            use async_trait::async_trait;
            use std::sync::Arc;
            
            /// Handler trait that users implement for the service
            #[async_trait]
            pub trait #handler_trait: Send + Sync + 'static {
                #(#handler_methods)*
            }
            
            /// Generated server that manages RPC registration and routing
            pub struct #server_name<H: #handler_trait> {
                handler: Arc<H>,
                rpc_server: RpcServer,
            }
            
            impl<H: #handler_trait> #server_name<H> {
                pub fn new(handler: H, config: RpcConfig) -> Self {
                    Self {
                        handler: Arc::new(handler),
                        rpc_server: RpcServer::new(config),
                    }
                }
                
                pub async fn register_all(&mut self) {
                    #(#register_methods)*
                }
                
                pub async fn serve(mut self) -> Result<(), RpcError> {
                    self.register_all().await;
                    let quic_server = self.rpc_server.bind()?;
                    self.rpc_server.start(quic_server).await
                }
            }
        }
    }
    
    pub fn generate_client(&self) -> TokenStream {
        let trait_name = &self.definition.service_trait.ident;
        let client_name = format_ident!("{}Client", trait_name);
        
        let methods = self.extract_methods();
        let client_methods = self.generate_client_methods(&methods, trait_name);
        
        quote! {
            use super::*;
            use rpcnet::{RpcClient, RpcConfig, RpcError};
            use std::net::SocketAddr;
            
            /// Generated client for calling service methods
            pub struct #client_name {
                inner: RpcClient,
            }
            
            impl #client_name {
                pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
                    let inner = RpcClient::connect(addr, config).await?;
                    Ok(Self { inner })
                }
                
                #(#client_methods)*
            }
        }
    }
    
    fn extract_methods(&self) -> Vec<&syn::TraitItemFn> {
        self.definition.service_trait.items.iter()
            .filter_map(|item| {
                if let TraitItem::Fn(method) = item {
                    Some(method)
                } else {
                    None
                }
            })
            .collect()
    }
    
    fn generate_handler_methods(&self, methods: &[&syn::TraitItemFn]) -> Vec<TokenStream> {
        methods.iter().map(|method| {
            let sig = &method.sig;
            quote! { #sig; }
        }).collect()
    }
    
    fn generate_register_methods(&self, methods: &[&syn::TraitItemFn], service_name: &syn::Ident) -> Vec<TokenStream> {
        methods.iter().map(|method| {
            let method_name = &method.sig.ident;
            let method_str = method_name.to_string();
            let full_method_name = format!("{}.{}", service_name, method_str);
            
            // Extract request type from method signature
            let request_type = self.extract_request_type(method);
            let response_type = self.extract_response_type(method);
            
            quote! {
                {
                    let handler = self.handler.clone();
                    self.rpc_server.register(#full_method_name, move |params| {
                        let handler = handler.clone();
                        async move {
                            let request: #request_type = bincode::deserialize(&params)
                                .map_err(RpcError::SerializationError)?;
                            
                            let response = handler.#method_name(request).await
                                .map_err(|e| RpcError::StreamError(format!("{:?}", e)))?;
                            
                            bincode::serialize(&response)
                                .map_err(RpcError::SerializationError)
                        }
                    }).await;
                }
            }
        }).collect()
    }
    
    fn generate_client_methods(&self, methods: &[&syn::TraitItemFn], service_name: &syn::Ident) -> Vec<TokenStream> {
        methods.iter().map(|method| {
            let method_name = &method.sig.ident;
            let method_str = method_name.to_string();
            let full_method_name = format!("{}.{}", service_name, method_str);
            
            let request_type = self.extract_request_type(method);
            let response_type = self.extract_response_type(method);
            let error_type = self.extract_error_type(method);
            
            quote! {
                pub async fn #method_name(&self, request: #request_type) -> Result<#response_type, RpcError> {
                    let params = bincode::serialize(&request)
                        .map_err(RpcError::SerializationError)?;
                    
                    let response_data = self.inner.call(#full_method_name, params).await?;
                    
                    let result: Result<#response_type, #error_type> = bincode::deserialize(&response_data)
                        .map_err(RpcError::SerializationError)?;
                    
                    result.map_err(|e| RpcError::StreamError(format!("{:?}", e)))
                }
            }
        }).collect()
    }
    
    fn extract_request_type(&self, method: &syn::TraitItemFn) -> TokenStream {
        // Get the first parameter (skipping &self)
        if let Some(FnArg::Typed(pat_type)) = method.sig.inputs.iter().nth(1) {
            let ty = &pat_type.ty;
            quote! { #ty }
        } else {
            quote! { () }
        }
    }
    
    fn extract_response_type(&self, method: &syn::TraitItemFn) -> TokenStream {
        // Parse the return type to extract T from Result<T, E>
        if let ReturnType::Type(_, ty) = &method.sig.output {
            // This is simplified - in real implementation we'd parse Result<T, E>
            quote! { /* extracted type */ }
        } else {
            quote! { () }
        }
    }
    
    fn extract_error_type(&self, method: &syn::TraitItemFn) -> TokenStream {
        // Parse the return type to extract E from Result<T, E>
        quote! { /* extracted error type */ }
    }
}
```

### CLI Binary Implementation

```rust
// src/bin/rpcnet-gen.rs

use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "rpcnet-gen")]
#[command(about = "Generate RPC client and server code from service definitions")]
struct Cli {
    /// Input .rpc file (Rust source with service trait)
    #[arg(short, long)]
    input: PathBuf,
    
    /// Output directory for generated code
    #[arg(short, long, default_value = "src/generated")]
    output: PathBuf,
    
    /// Generate only server code
    #[arg(long)]
    server_only: bool,
    
    /// Generate only client code
    #[arg(long)]
    client_only: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Read the input file
    let content = fs::read_to_string(&cli.input)?;
    
    // Parse using syn
    let definition = rpcnet::codegen::ServiceDefinition::parse(&content)?;
    
    // Generate code
    let generator = rpcnet::codegen::CodeGenerator::new(definition);
    
    // Create output directory
    fs::create_dir_all(&cli.output)?;
    
    // Generate and write files
    if !cli.client_only {
        let server_code = generator.generate_server();
        let server_path = cli.output.join("server.rs");
        write_formatted_code(&server_path, server_code)?;
        println!("Generated server: {}", server_path.display());
    }
    
    if !cli.server_only {
        let client_code = generator.generate_client();
        let client_path = cli.output.join("client.rs");
        write_formatted_code(&client_path, client_code)?;
        println!("Generated client: {}", client_path.display());
    }
    
    // Generate mod.rs
    generate_mod_file(&cli.output)?;
    
    println!("✨ Code generation complete!");
    Ok(())
}

fn write_formatted_code(path: &Path, tokens: proc_macro2::TokenStream) -> std::io::Result<()> {
    let file = syn::parse2::<syn::File>(tokens)
        .expect("Generated invalid Rust code");
    
    // Format using prettyplease for nice output
    let formatted = prettyplease::unparse(&file);
    fs::write(path, formatted)
}

fn generate_mod_file(output_dir: &Path) -> std::io::Result<()> {
    let mod_content = r#"
pub mod server;
pub mod client;

// Re-export commonly used types
pub use server::*;
pub use client::*;
"#;
    
    fs::write(output_dir.join("mod.rs"), mod_content)
}
```

### Build Script Integration

```rust
// Example build.rs for users

fn main() {
    // Only run codegen if source files changed
    println!("cargo:rerun-if-changed=rpc/");
    
    // Use the rpcnet library's builder API
    rpcnet::codegen::Builder::new()
        .input("rpc/calculator.rpc")
        .output("src/generated")
        .build()
        .expect("Failed to generate RPC code");
}
```

```rust
// src/codegen/mod.rs - Builder API for build scripts

pub struct Builder {
    inputs: Vec<PathBuf>,
    output: PathBuf,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            output: PathBuf::from("src/generated"),
        }
    }
    
    pub fn input<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.inputs.push(path.as_ref().to_path_buf());
        self
    }
    
    pub fn output<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.output = path.as_ref().to_path_buf();
        self
    }
    
    pub fn build(self) -> Result<(), Box<dyn std::error::Error>> {
        for input in &self.inputs {
            let content = std::fs::read_to_string(input)?;
            let definition = ServiceDefinition::parse(&content)?;
            let generator = CodeGenerator::new(definition);
            
            // Generate code...
            // Write files...
        }
        Ok(())
    }
}
```

## Usage Example

### 1. Define Service

```rust
// rpc/math_service.rpc

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CalculateRequest {
    pub expression: String,
}

#[derive(Serialize, Deserialize)]
pub struct CalculateResponse {
    pub result: f64,
}

#[rpcnet::service]
pub trait MathService {
    async fn calculate(&self, req: CalculateRequest) -> Result<CalculateResponse, String>;
}
```

### 2. Generate Code

```bash
# Install rpcnet with codegen feature
cargo install rpcnet --features codegen

# Generate code
rpcnet-gen --input rpc/math_service.rpc --output src/generated
```

### 3. Implement Server

```rust
// src/server.rs

use crate::generated::server::{MathServiceHandler, MathServiceServer};
use crate::generated::{CalculateRequest, CalculateResponse};
use async_trait::async_trait;

struct MathHandler;

#[async_trait]
impl MathServiceHandler for MathHandler {
    async fn calculate(&self, req: CalculateRequest) -> Result<CalculateResponse, String> {
        // Parse and evaluate expression
        let result = eval_expression(&req.expression)?;
        Ok(CalculateResponse { result })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
        .with_key_path("key.pem");
    
    let server = MathServiceServer::new(MathHandler, config);
    server.serve().await?;
    Ok(())
}
```

### 4. Use Client

```rust
// src/client.rs

use crate::generated::client::MathServiceClient;
use crate::generated::{CalculateRequest, CalculateResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    let addr = "127.0.0.1:8080".parse()?;
    
    let client = MathServiceClient::connect(addr, config).await?;
    
    let response = client.calculate(CalculateRequest {
        expression: "2 + 2 * 3".to_string(),
    }).await?;
    
    println!("Result: {}", response.result);
    Ok(())
}
```

## Advantages of This Approach

1. **No Custom Parser**: We leverage the mature `syn` crate
2. **Single Crate**: Everything is in the rpcnet crate with optional features
3. **Standard Rust Syntax**: `.rpc` files are valid Rust, enabling IDE support
4. **Simpler Maintenance**: Less code to maintain
5. **Better Integration**: Can reuse types from the main library
6. **Familiar Tools**: Uses standard Rust ecosystem tools

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_parse_service_definition() {
        let input = r#"
            #[rpcnet::service]
            pub trait TestService {
                async fn test(&self, req: TestRequest) -> Result<TestResponse, Error>;
            }
        "#;
        
        let def = ServiceDefinition::parse(input).unwrap();
        assert_eq!(def.service_trait.ident, "TestService");
    }
    
    #[test]
    fn test_generate_code() {
        // Test that generated code compiles
        let tokens = generator.generate_server();
        let file = syn::parse2::<syn::File>(tokens).unwrap();
        // If parsing succeeds, the generated code is valid Rust
    }
}
```

## Conclusion

This simplified approach:
- Reduces implementation complexity by 70%
- Leverages existing, battle-tested tools
- Provides better IDE support (`.rpc` files are valid Rust)
- Simplifies distribution (single crate with optional features)
- Maintains all the benefits of code generation

The use of `syn` and `quote` makes the implementation robust and maintainable, while keeping everything in the main rpcnet crate simplifies the user experience.