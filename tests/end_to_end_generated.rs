//! End-to-end test of the generated code.

#![cfg(feature = "codegen")]

use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_generated_calculator_service() {
    // Generate code in a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let service_dir = temp_dir.path().join("calculator");

    // Use the builder to generate code
    rpcnet::codegen::Builder::new()
        .input("examples/calculator/calculator.rpc.rs")
        .output(temp_dir.path())
        .build()
        .expect("Failed to generate code");

    // Verify that the generated files exist and are valid Rust
    assert!(service_dir.join("mod.rs").exists());
    assert!(service_dir.join("types.rs").exists());
    assert!(service_dir.join("server.rs").exists());
    assert!(service_dir.join("client.rs").exists());

    // Read and verify the generated server code can be parsed
    let server_code = std::fs::read_to_string(service_dir.join("server.rs")).unwrap();
    let server_ast =
        syn::parse_file(&server_code).expect("Generated server code is not valid Rust");

    // Check that the server contains the expected trait and struct
    let has_handler_trait = server_code.contains("trait CalculatorHandler");
    let has_server_struct = server_code.contains("struct CalculatorServer");
    let has_register_all = server_code.contains("register_all");

    assert!(
        has_handler_trait,
        "Generated server should have CalculatorHandler trait"
    );
    assert!(
        has_server_struct,
        "Generated server should have CalculatorServer struct"
    );
    assert!(
        has_register_all,
        "Generated server should have register_all method"
    );

    // Read and verify the generated client code
    let client_code = std::fs::read_to_string(service_dir.join("client.rs")).unwrap();
    let client_ast =
        syn::parse_file(&client_code).expect("Generated client code is not valid Rust");

    // Check that the client contains the expected struct and methods
    let has_client_struct = client_code.contains("struct CalculatorClient");
    let has_connect = client_code.contains("async fn connect");
    let has_add_method = client_code.contains("async fn add");
    let has_divide_method = client_code.contains("async fn divide");

    assert!(
        has_client_struct,
        "Generated client should have CalculatorClient struct"
    );
    assert!(has_connect, "Generated client should have connect method");
    assert!(has_add_method, "Generated client should have add method");
    assert!(
        has_divide_method,
        "Generated client should have divide method"
    );

    // Read and verify types
    let types_code = std::fs::read_to_string(service_dir.join("types.rs")).unwrap();
    let types_ast = syn::parse_file(&types_code).expect("Generated types code is not valid Rust");

    let has_add_request = types_code.contains("struct AddRequest");
    let has_calculator_error = types_code.contains("enum CalculatorError");

    assert!(
        has_add_request,
        "Generated types should have AddRequest struct"
    );
    assert!(
        has_calculator_error,
        "Generated types should have CalculatorError enum"
    );

    println!("✅ All generated code files are valid and contain expected elements!");
}

#[tokio::test]
async fn test_cli_generation() {
    let temp_dir = TempDir::new().unwrap();

    // Test the CLI by running it as a subprocess
    let output = std::process::Command::new("cargo")
        .args(&[
            "run",
            "--features",
            "codegen",
            "--bin",
            "rpcnet-gen",
            "--",
            "--input",
            "examples/calculator/calculator.rpc.rs",
            "--output",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run rpcnet-gen CLI");

    if !output.status.success() {
        panic!(
            "CLI command failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Verify output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Code generation complete!"));
    assert!(stdout.contains("Generated server:"));
    assert!(stdout.contains("Generated client:"));
    assert!(stdout.contains("Generated types:"));

    // Verify files were created
    let service_dir = temp_dir.path().join("calculator");
    assert!(service_dir.exists());
    assert!(service_dir.join("mod.rs").exists());
    assert!(service_dir.join("server.rs").exists());
    assert!(service_dir.join("client.rs").exists());
    assert!(service_dir.join("types.rs").exists());

    println!("✅ CLI generation works correctly!");
}
