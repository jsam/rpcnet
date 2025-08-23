//! Echo client using generated code.

#[path = "generated/echo/mod.rs"]
mod echo;

use echo::{EchoRequest, BinaryEchoRequest};
use echo::client::EchoClient;
use rpcnet::RpcConfig;
use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Echo Client (Generated Code) ===");
    
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(30));
    
    let server_addr: SocketAddr = "127.0.0.1:8081".parse()?;
    
    println!("Connecting to echo service at {}", server_addr);
    let client = EchoClient::connect(server_addr, config).await?;
    println!("Connected successfully!");
    
    // Test basic echo
    println!("\n--- Testing Text Echo ---");
    let echo_req = EchoRequest { message: "Hello World".to_string(), times: 1 };
    match client.echo(echo_req).await {
        Ok(response) => println!("Echo: {}", response.echoed_message),
        Err(e) => println!("Echo failed: {}", e),
    }
    
    // Test multiple echo
    let echo_req = EchoRequest { message: "Test".to_string(), times: 3 };
    match client.echo(echo_req).await {
        Ok(response) => println!("Multiple echo: {}", response.echoed_message),
        Err(e) => println!("Multiple echo failed: {}", e),
    }
    
    // Test binary echo
    println!("\n--- Testing Binary Echo ---");
    let binary_data = b"Binary test data 123".to_vec();
    let binary_req = BinaryEchoRequest { data: binary_data.clone() };
    match client.binary_echo(binary_req).await {
        Ok(response) => {
            if response.data == binary_data {
                println!("Binary echo successful: {} bytes returned", response.data.len());
            } else {
                println!("Binary echo mismatch!");
            }
        }
        Err(e) => println!("Binary echo failed: {}", e),
    }
    
    // Test error handling (too many repetitions)
    println!("\n--- Testing Error Handling ---");
    let echo_req = EchoRequest { message: "Error".to_string(), times: 200 };
    match client.echo(echo_req).await {
        Ok(response) => println!("Unexpected success: {}", response.echoed_message),
        Err(e) => println!("Error handling works: {}", e),
    }
    
    println!("\nAll tests completed!");
    Ok(())
}