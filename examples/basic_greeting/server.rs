#![allow(dead_code)]
#![allow(unused_imports)]
//! Basic greeting server using generated code.

#[path = "generated/greeting/mod.rs"]
mod greeting;

use greeting::server::{GreetingHandler, GreetingServer};
use greeting::{GreetRequest, GreetResponse, GreetingError};
use rpcnet::RpcConfig;

struct MyGreetingService;

#[async_trait::async_trait]
impl GreetingHandler for MyGreetingService {
    async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError> {
        if request.name.trim().is_empty() {
            return Err(GreetingError::EmptyName);
        }

        let message = format!("Hello, {}!", request.name);
        Ok(GreetResponse { message })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic Greeting Server (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8080")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let server = GreetingServer::new(MyGreetingService, config);
    println!("Starting greeting server...");

    server.serve().await?;
    Ok(())
}
