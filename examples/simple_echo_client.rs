//! Echo client using the low-level API.
//!
//! This example demonstrates various echo operations including text repetition,
//! binary data handling, and string reversal.

use rpcnet::{RpcClient, RpcConfig};
use serde::{Serialize, Deserialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug)]
struct EchoRequest {
    message: String,
    times: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct EchoResponse {
    echoed_message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Echo Client ===");
    
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_server_name("localhost");
    
    let server_addr: SocketAddr = "127.0.0.1:8081".parse()?;
    
    println!("Connecting to echo server at {}...", server_addr);
    let client = RpcClient::connect(server_addr, config).await?;
    println!("Connected successfully!");
    
    // Test basic echo
    println!("\n--- Testing Text Echo ---");
    let request = EchoRequest {
        message: "Hello Echo".to_string(),
        times: 1,
    };
    let params = bincode::serialize(&request)?;
    let response_bytes = client.call("echo", params).await?;
    let response: EchoResponse = bincode::deserialize(&response_bytes)?;
    println!("Echo: {}", response.echoed_message);
    
    // Test multiple echo
    let request = EchoRequest {
        message: "Test".to_string(),
        times: 3,
    };
    let params = bincode::serialize(&request)?;
    let response_bytes = client.call("echo", params).await?;
    let response: EchoResponse = bincode::deserialize(&response_bytes)?;
    println!("Multiple echo (3x): {}", response.echoed_message);
    
    // Test binary echo
    println!("\n--- Testing Binary Echo ---");
    let binary_data = b"Binary test data \x00\x01\x02\x03";
    let response_bytes = client.call("binary_echo", binary_data.to_vec()).await?;
    
    if response_bytes == binary_data {
        println!("✅ Binary echo successful: {} bytes", response_bytes.len());
    } else {
        println!("❌ Binary echo failed");
    }
    
    // Test large binary data
    let large_data = vec![0xAB; 50_000];  // 50KB
    let response_bytes = client.call("binary_echo", large_data.clone()).await?;
    println!("Large binary echo: {} bytes (expected {})", 
        response_bytes.len(), large_data.len());
    
    // Test reverse
    println!("\n--- Testing Reverse ---");
    let test_text = "Hello World!";
    let response_bytes = client.call("reverse", test_text.as_bytes().to_vec()).await?;
    let reversed = String::from_utf8_lossy(&response_bytes);
    println!("Original: {}", test_text);
    println!("Reversed: {}", reversed);
    
    // Test error handling (too many repetitions)
    println!("\n--- Testing Error Handling ---");
    let request = EchoRequest {
        message: "Error".to_string(),
        times: 200,  // Should exceed limit
    };
    let params = bincode::serialize(&request)?;
    match client.call("echo", params).await {
        Ok(_) => println!("❌ Expected error but got success"),
        Err(e) => println!("✅ Error handling works: {}", e),
    }
    
    println!("\nAll tests completed!");
    Ok(())
}