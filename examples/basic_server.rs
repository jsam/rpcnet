//! Basic RPC server using the low-level API.
//!
//! This example demonstrates how to create a simple RPC server without code generation.
//! It shows manual method registration and binary serialization handling.

use rpcnet::{RpcConfig, RpcServer};
use serde::{Deserialize, Serialize};

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
    println!("=== Basic RPC Server ===");

    // Configure server with TLS
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8080")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);

    // Register greeting method
    server
        .register("greet", |params| async move {
            // Deserialize request
            let request: GreetRequest = rmp_serde::from_slice(&params)?;

            // Process request
            let response = if request.name.trim().is_empty() {
                GreetResponse {
                    message: "Hello, anonymous!".to_string(),
                }
            } else {
                GreetResponse {
                    message: format!("Hello, {}!", request.name),
                }
            };

            // Serialize response
            rmp_serde::to_vec(&response).map_err(Into::into)
        })
        .await;

    // Register echo method for binary data
    server
        .register("echo", |params| async move {
            // Just echo back the same data
            Ok(params)
        })
        .await;

    // Start server
    println!("Starting server on 127.0.0.1:8080...");
    println!("Methods available: greet, echo");
    println!("Use Ctrl+C to stop");

    let quic_server = server.bind()?;
    server.start(quic_server).await?;

    Ok(())
}
