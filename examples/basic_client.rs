//! Basic RPC client using the low-level API.
//!
//! This example demonstrates how to create a simple RPC client without code generation.
//! It shows manual serialization and method calling with string method names.

use rpcnet::{RpcClient, RpcConfig};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug)]
struct GreetRequest {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GreetResponse {
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic RPC Client ===");

    // Configure client
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_server_name("localhost");

    let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;

    // Connect to server
    println!("Connecting to server at {}...", server_addr);
    let client = RpcClient::connect(server_addr, config).await?;
    println!("Connected successfully!");

    // Test greeting with name
    println!("\n--- Testing Greeting ---");
    let request = GreetRequest {
        name: "World".to_string(),
    };
    let params = bincode::serialize(&request)?;
    let response_bytes = client.call("greet", params).await?;
    let response: GreetResponse = bincode::deserialize(&response_bytes)?;
    println!("Response: {}", response.message);

    // Test greeting with empty name
    let request = GreetRequest {
        name: "".to_string(),
    };
    let params = bincode::serialize(&request)?;
    let response_bytes = client.call("greet", params).await?;
    let response: GreetResponse = bincode::deserialize(&response_bytes)?;
    println!("Empty name response: {}", response.message);

    // Test echo with binary data
    println!("\n--- Testing Echo ---");
    let test_data = b"Hello, binary world!";
    let response_bytes = client.call("echo", test_data.to_vec()).await?;
    let response_str = String::from_utf8_lossy(&response_bytes);
    println!("Echo response: {}", response_str);

    // Test with larger data
    let large_data = vec![42u8; 1000];
    let response_bytes = client.call("echo", large_data.clone()).await?;
    println!(
        "Large data echo: {} bytes returned (expected {})",
        response_bytes.len(),
        large_data.len()
    );

    if response_bytes == large_data {
        println!("✅ Large data echo successful!");
    } else {
        println!("❌ Large data echo failed!");
    }

    println!("\nAll tests completed!");
    Ok(())
}
