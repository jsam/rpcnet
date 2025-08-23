//! Echo server using the low-level API.
//!
//! This example demonstrates a simple echo service that handles both text and binary data
//! without requiring code generation. Perfect for testing connectivity and data integrity.

use rpcnet::{RpcServer, RpcConfig, RpcError};
use serde::{Serialize, Deserialize};

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
    println!("=== Echo Server ===");
    
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8081")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");
    
    let mut server = RpcServer::new(config);
    
    // Text echo with repetition
    server.register("echo", |params| async move {
        let request: EchoRequest = bincode::deserialize(&params)
            .map_err(RpcError::SerializationError)?;
        
        if request.times > 100 {
            return Err(RpcError::StreamError("Too many repetitions".to_string()));
        }
        
        let echoed_message = if request.times <= 1 {
            request.message
        } else {
            (0..request.times)
                .map(|_| request.message.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };
        
        let response = EchoResponse { echoed_message };
        bincode::serialize(&response)
            .map_err(RpcError::SerializationError)
    }).await;
    
    // Binary echo
    server.register("binary_echo", |params| async move {
        if params.len() > 1024 * 1024 {  // 1MB limit
            return Err(RpcError::StreamError("Data too large".to_string()));
        }
        Ok(params)  // Just echo back the raw bytes
    }).await;
    
    // Reverse echo
    server.register("reverse", |params| async move {
        let text = String::from_utf8_lossy(&params);
        let reversed: String = text.chars().rev().collect();
        Ok(reversed.into_bytes())
    }).await;
    
    println!("Starting echo server on port 8081...");
    println!("Available methods:");
    println!("  - echo: Text echo with repetition");
    println!("  - binary_echo: Raw binary data echo");  
    println!("  - reverse: Reverse text");
    println!("Use Ctrl+C to stop");
    
    let quic_server = server.bind()?;
    server.start(quic_server).await?;
    
    Ok(())
}