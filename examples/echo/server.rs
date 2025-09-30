//! Echo server using generated code.

#[path = "generated/echo/mod.rs"]
mod echo;

use echo::server::{EchoHandler, EchoServer};
use echo::{BinaryEchoRequest, BinaryEchoResponse, EchoError, EchoRequest, EchoResponse};
use rpcnet::RpcConfig;
use std::time::Duration;

struct MyEchoService;

#[async_trait::async_trait]
impl EchoHandler for MyEchoService {
    async fn echo(&self, request: EchoRequest) -> Result<EchoResponse, EchoError> {
        if request.message.is_empty() {
            return Err(EchoError::EmptyMessage);
        }

        if request.times > 100 {
            return Err(EchoError::TooManyRepetitions);
        }

        let echoed_message = if request.times <= 1 {
            request.message
        } else {
            (0..request.times)
                .map(|_| request.message.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };

        Ok(EchoResponse { echoed_message })
    }

    async fn binary_echo(
        &self,
        request: BinaryEchoRequest,
    ) -> Result<BinaryEchoResponse, EchoError> {
        if request.data.len() > 1024 * 1024 {
            // 1MB limit
            return Err(EchoError::DataTooLarge);
        }

        Ok(BinaryEchoResponse { data: request.data })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Echo Server (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8081")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(30));

    let server = EchoServer::new(MyEchoService, config);
    println!("Starting echo server on port 8081...");

    server.serve().await?;
    Ok(())
}
