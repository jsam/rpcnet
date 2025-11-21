use anyhow::Result;
use rpcnet::{RpcConfig, RpcError, RpcServer};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::time::SystemTime;
use tracing::info;

// Type definitions matching streaming.rpc.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryRequest {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryResponse {
    pub reply: String,
    pub timestamp: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("streaming_example=info".parse()?),
        )
        .init();

    let addr: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50052".to_string())
        .parse()?;

    info!("🚀 Starting Streaming RPC Server at {}", addr);

    let cert_path = if Path::new("../../../certs/test_cert.pem").exists() {
        Path::new("../../../certs/test_cert.pem")
    } else {
        panic!("Cannot find test_cert.pem - run ./generate_certs.sh from repo root");
    };

    let key_path = if Path::new("../../../certs/test_key.pem").exists() {
        Path::new("../../../certs/test_key.pem")
    } else {
        panic!("Cannot find test_key.pem - run ./generate_certs.sh from repo root");
    };

    info!("📁 Using certificates: {:?}, {:?}", cert_path, key_path);

    let config = RpcConfig::new(cert_path, addr.to_string())
        .with_key_path(key_path)
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);

    // Register unary handler (basic RPC - no streaming yet)
    let unary_handler = move |request: UnaryRequest| async move {
        info!("📨 Unary request: {}", request.message);
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        Ok::<UnaryResponse, RpcError>(UnaryResponse {
            reply: format!("Server received: {}", request.message),
            timestamp,
        })
    };
    server.register_typed_polyglot("StreamingService.unary", unary_handler).await;

    info!("✅ Server ready:");
    info!("   📨 StreamingService.unary - Single request/response");
    info!("");
    info!("Note: Full streaming support (server_stream, client_stream, bidi_stream)");
    info!("      will be added in future releases. For now, basic unary RPC works.");

    let srv = server.bind()?;
    server.start(srv).await?;

    Ok(())
}
