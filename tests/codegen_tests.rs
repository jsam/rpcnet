//! Integration tests for code generation.

#![cfg(feature = "codegen")]

use rpcnet::codegen::{CodeGenerator, ServiceDefinition};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test that we can parse a simple service definition.
#[test]
fn test_parse_simple_service() {
    let input = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct AddRequest {
            pub a: i32,
            pub b: i32,
        }
        
        #[derive(Serialize, Deserialize)]
        pub struct AddResponse {
            pub result: i32,
        }
        
        #[derive(Serialize, Deserialize)]
        pub enum MathError {
            Overflow,
        }
        
        #[service]
        pub trait Calculator {
            async fn add(&self, request: AddRequest) -> Result<AddResponse, MathError>;
        }
    "#;

    let definition = ServiceDefinition::parse(input).expect("Failed to parse");
    assert_eq!(definition.service_name().to_string(), "Calculator");

    let methods = definition.methods();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].sig.ident.to_string(), "add");
}

/// Test that we reject invalid service definitions.
#[test]
fn test_reject_non_async_methods() {
    let input = r#"
        #[service]
        pub trait BadService {
            fn sync_method(&self) -> String;
        }
    "#;

    let result = ServiceDefinition::parse(input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("must be async"));
}

/// Test that we reject services without Result return types.
#[test]
fn test_reject_non_result_return() {
    let input = r#"
        #[service]
        pub trait BadService {
            async fn bad_method(&self) -> String;
        }
    "#;

    let result = ServiceDefinition::parse(input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("must return Result"));
}

/// Test that we can generate server code.
#[test]
fn test_generate_server_code() {
    let input = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct EchoRequest {
            pub message: String,
        }
        
        #[derive(Serialize, Deserialize)]
        pub struct EchoResponse {
            pub message: String,
        }
        
        #[service]
        pub trait EchoService {
            async fn echo(&self, request: EchoRequest) -> Result<EchoResponse, String>;
        }
    "#;

    let definition = ServiceDefinition::parse(input).expect("Failed to parse");
    let generator = CodeGenerator::new(definition);

    let server_code = generator.generate_server();
    let server_str = server_code.to_string();

    // Check that generated code contains expected elements
    assert!(server_str.contains("EchoServiceHandler"));
    assert!(server_str.contains("EchoServiceServer"));
    assert!(server_str.contains("async fn echo"));
    assert!(server_str.contains("register_all"));
    assert!(server_str.contains("serve"));
}

/// Test that we can generate client code.
#[test]
fn test_generate_client_code() {
    let input = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct PingRequest {
            pub id: u64,
        }
        
        #[derive(Serialize, Deserialize)]
        pub struct PingResponse {
            pub id: u64,
            pub timestamp: u64,
        }
        
        #[service]
        pub trait PingService {
            async fn ping(&self, request: PingRequest) -> Result<PingResponse, String>;
        }
    "#;

    let definition = ServiceDefinition::parse(input).expect("Failed to parse");
    let generator = CodeGenerator::new(definition);

    let client_code = generator.generate_client();
    let client_str = client_code.to_string();

    // Check that generated code contains expected elements
    assert!(client_str.contains("PingServiceClient"));
    assert!(client_str.contains("async fn connect"));
    assert!(client_str.contains("async fn ping"));
    // Check for RPC call - should be self.inner.call
    assert!(
        client_str.contains("self . inner . call") || client_str.contains("self.inner.call"),
        "Generated code should contain a call to the inner RPC client"
    );
}

/// Test end-to-end code generation with Builder API.
#[test]
fn test_builder_api() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test_service.rpc.rs");
    let output_dir = temp_dir.path().join("generated");

    // Write test service definition
    let service_def = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct TestRequest {
            pub value: String,
        }
        
        #[derive(Serialize, Deserialize)]
        pub struct TestResponse {
            pub result: String,
        }
        
        #[service]
        pub trait TestService {
            async fn test_method(&self, request: TestRequest) -> Result<TestResponse, String>;
        }
    "#;

    fs::write(&input_file, service_def).unwrap();

    // Use builder to generate code
    rpcnet::codegen::Builder::new()
        .input(&input_file)
        .output(&output_dir)
        .build()
        .expect("Failed to generate code");

    // Check that files were created
    let service_dir = output_dir.join("test_service");
    assert!(service_dir.exists());
    assert!(service_dir.join("mod.rs").exists());
    assert!(service_dir.join("types.rs").exists());
    assert!(service_dir.join("server.rs").exists());
    assert!(service_dir.join("client.rs").exists());

    // Read and verify generated mod.rs
    let mod_content = fs::read_to_string(service_dir.join("mod.rs")).unwrap();
    assert!(mod_content.contains("pub mod types"));
    assert!(mod_content.contains("pub mod server"));
    assert!(mod_content.contains("pub mod client"));

    // Read and verify types.rs contains our types
    let types_content = fs::read_to_string(service_dir.join("types.rs")).unwrap();
    assert!(types_content.contains("struct TestRequest"));
    assert!(types_content.contains("struct TestResponse"));
}

/// Test parsing the example calculator service.
#[test]
fn test_parse_calculator_example() {
    let calculator_path = PathBuf::from("examples/calculator/calculator.rpc.rs");
    if calculator_path.exists() {
        let content = fs::read_to_string(calculator_path).unwrap();
        let definition =
            ServiceDefinition::parse(&content).expect("Failed to parse calculator.rpc.rs");

        assert_eq!(definition.service_name().to_string(), "Calculator");

        let methods = definition.methods();
        assert_eq!(methods.len(), 4);

        let method_names: Vec<String> = methods.iter().map(|m| m.sig.ident.to_string()).collect();
        assert!(method_names.contains(&"add".to_string()));
        assert!(method_names.contains(&"subtract".to_string()));
        assert!(method_names.contains(&"multiply".to_string()));
        assert!(method_names.contains(&"divide".to_string()));
    }
}

/// Test that generated code compiles (by parsing it with syn).
#[test]
fn test_generated_code_is_valid_rust() {
    let input = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct Request {
            pub data: Vec<u8>,
        }
        
        #[derive(Serialize, Deserialize)]
        pub struct Response {
            pub result: Vec<u8>,
        }
        
        #[service]
        pub trait DataService {
            async fn process(&self, request: Request) -> Result<Response, String>;
        }
    "#;

    let definition = ServiceDefinition::parse(input).expect("Failed to parse");
    let generator = CodeGenerator::new(definition);

    // Generate all code
    let server_code = generator.generate_server();
    let client_code = generator.generate_client();
    let types_code = generator.generate_types();

    // Try to parse generated code as valid Rust
    syn::parse2::<syn::File>(server_code).expect("Generated server code is not valid Rust");
    syn::parse2::<syn::File>(client_code).expect("Generated client code is not valid Rust");
    syn::parse2::<syn::File>(types_code).expect("Generated types code is not valid Rust");
}

/// Test handling of multiple methods.
#[test]
fn test_multiple_methods() {
    let input = r#"
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        pub struct GetRequest { pub key: String }
        
        #[derive(Serialize, Deserialize)]
        pub struct GetResponse { pub value: String }
        
        #[derive(Serialize, Deserialize)]
        pub struct SetRequest { pub key: String, pub value: String }
        
        #[derive(Serialize, Deserialize)]
        pub struct SetResponse { pub success: bool }
        
        #[derive(Serialize, Deserialize)]
        pub struct DeleteRequest { pub key: String }
        
        #[derive(Serialize, Deserialize)]
        pub struct DeleteResponse { pub success: bool }
        
        #[service]
        pub trait KVStore {
            async fn get(&self, request: GetRequest) -> Result<GetResponse, String>;
            async fn set(&self, request: SetRequest) -> Result<SetResponse, String>;
            async fn delete(&self, request: DeleteRequest) -> Result<DeleteResponse, String>;
        }
    "#;

    let definition = ServiceDefinition::parse(input).expect("Failed to parse");
    assert_eq!(definition.methods().len(), 3);

    let generator = CodeGenerator::new(definition);
    let server_code = generator.generate_server();
    let server_str = server_code.to_string();

    // Check all methods are present
    assert!(server_str.contains("async fn get"));
    assert!(server_str.contains("async fn set"));
    assert!(server_str.contains("async fn delete"));
    assert!(server_str.contains("KVStore.get"));
    assert!(server_str.contains("KVStore.set"));
    assert!(server_str.contains("KVStore.delete"));
}
