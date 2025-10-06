#![allow(dead_code)]
#![allow(unused_imports)]
//! Basic greeting client using generated code.

#[path = "generated/greeting/mod.rs"]
mod greeting;

use greeting::client::GreetingClient;
use greeting::GreetRequest;
use rpcnet::RpcConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic Greeting Client (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_server_name("localhost");

    let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;

    println!("Connecting to greeting service at {}", server_addr);
    let client = GreetingClient::connect(server_addr, config).await?;
    println!("Connected successfully!");

    // Test basic greeting
    println!("\n--- Testing Greetings ---");
    let greet_req = GreetRequest {
        name: "World".to_string(),
    };
    match client.greet(greet_req).await {
        Ok(response) => println!("Response: {}", response.message),
        Err(e) => println!("Greeting failed: {}", e),
    }

    // Test with different name
    let greet_req = GreetRequest {
        name: "RPC Developer".to_string(),
    };
    match client.greet(greet_req).await {
        Ok(response) => println!("Response: {}", response.message),
        Err(e) => println!("Greeting failed: {}", e),
    }

    // Test empty name (should fail)
    let greet_req = GreetRequest {
        name: "".to_string(),
    };
    match client.greet(greet_req).await {
        Ok(response) => println!("Response: {}", response.message),
        Err(e) => println!("Empty name handled correctly: {}", e),
    }

    println!("\nAll tests completed!");
    Ok(())
}
