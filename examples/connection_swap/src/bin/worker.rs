use anyhow::Result;
use connection_swap::protocol::*;
use rpcnet::{RpcServer, RpcClient, RpcConfig, RpcError};
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::{sleep, Instant};
use tracing::{info, warn, error};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let worker_label = env::var("CONNECTION_SWAP_WORKER_LABEL")
        .unwrap_or_else(|_| "worker-unknown".to_string());
    let user_addr = env::var("CONNECTION_SWAP_WORKER_USER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:62001".to_string());
    let mgmt_addr = env::var("CONNECTION_SWAP_WORKER_MGMT_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:63001".to_string());
    let director_mgmt_target = env::var("CONNECTION_SWAP_DIRECTOR_MGMT_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61001".to_string());

    info!(
        worker = %worker_label,
        user.port = %user_addr,
        mgmt.port = %mgmt_addr,
        "🚀 starting worker with dual-port architecture"
    );

    // Shared availability state
    let is_available = Arc::new(RwLock::new(true));

    // ═══════════════════════════════════════════════════════════════
    // USER PORT - RPC endpoints for actual work
    // ═══════════════════════════════════════════════════════════════
    let user_config = RpcConfig::new("../../certs/test_cert.pem", user_addr.clone())
        .with_key_path("../../certs/test_key.pem");
    let mut user_server = RpcServer::new(user_config);

    // ═══════════════════════════════════════════════════════════════
    // MANAGEMENT PORT - Control plane for health checks, registration
    // ═══════════════════════════════════════════════════════════════
    let mgmt_config = RpcConfig::new("../../certs/test_cert.pem", mgmt_addr.clone())
        .with_key_path("../../certs/test_key.pem");
    let mut mgmt_server = RpcServer::new(mgmt_config);

    // Health check endpoint on MANAGEMENT port
    let label_for_health = worker_label.clone();
    mgmt_server.register("health_check", move |params: Vec<u8>| {
        let label = label_for_health.clone();
        async move {
            match bincode::deserialize::<HealthCheckRequest>(&params) {
                Ok(_req) => {
                    let response = HealthCheckResponse {
                        healthy: true,
                        worker_label: label.clone(),
                        timestamp: SystemTime::now(),
                    };
                    bincode::serialize(&response).map_err(RpcError::SerializationError)
                }
                Err(e) => Err(RpcError::SerializationError(e)),
            }
        }
    }).await;

    // Streaming handler for inference requests on USER port
    let label_clone = worker_label.clone();
    let availability_for_streaming = is_available.clone();
    user_server.register_streaming("generate", move |request_stream| {
        let label = label_clone.clone();
        let availability = availability_for_streaming.clone();
        async move {
            info!(worker = %label, "🎬 streaming handler invoked");
            async_stream::stream! {
                let mut request_stream = Box::pin(request_stream);
                info!(worker = %label, "⏳ waiting for request bytes from stream");
                if let Some(request_bytes) = request_stream.next().await {
                    info!(worker = %label, "📦 received {} bytes from client", request_bytes.len());
                    match bincode::deserialize::<InferenceRequest>(&request_bytes) {
                        Ok(inf_req) => {
                            let connection_id = inf_req.connection_id.clone();
                            
                            info!(
                                worker = %label,
                                connection.id = %connection_id,
                                prompt = %inf_req.prompt,
                                "✅ received inference request from client (direct connection)"
                            );
                            
                            // Send Connected response
                            let connected_response = InferenceResponse::Connected {
                                worker: label.clone(),
                                connection_id: connection_id.clone(),
                            };
                            if let Ok(bytes) = bincode::serialize(&connected_response) {
                                yield Ok(bytes);
                            }

                            let start_time = Instant::now();
                            let failure_after = Duration::from_secs(15);
                            let token_interval = Duration::from_millis(500);
                            let mut sequence = 0u64;

                            loop {
                                let elapsed = start_time.elapsed();
                                
                                if elapsed >= failure_after {
                                    warn!(
                                        worker = %label,
                                        connection.id = %connection_id,
                                        "⚠️  simulating failure after {:?}", failure_after
                                    );

                                    let error_response = InferenceResponse::Error {
                                        message: format!("{} simulated failure - will recover in 10s", label),
                                    };

                                    if let Ok(bytes) = bincode::serialize(&error_response) {
                                        yield Ok(bytes);
                                    }
                                    
                                    // Mark worker as unavailable
                                    {
                                        let mut avail = availability.write().await;
                                        *avail = false;
                                    }
                                    
                                    info!(worker = %label, "💤 worker entering recovery mode (10s cooldown) - marked as unavailable");
                                    sleep(Duration::from_secs(10)).await;
                                    
                                    // Mark worker as available again
                                    {
                                        let mut avail = availability.write().await;
                                        *avail = true;
                                    }
                                    
                                    info!(worker = %label, "✅ worker recovered and ready for new connections - marked as available");
                                    
                                    return;
                                }

                                sleep(token_interval).await;

                                sequence += 1;
                                let token_response = InferenceResponse::Token {
                                    text: format!("token-{}", sequence),
                                    sequence,
                                };

                                info!(
                                    worker = %label,
                                    connection.id = %connection_id,
                                    seq = sequence,
                                    "📤 sending token"
                                );

                                match bincode::serialize(&token_response) {
                                    Ok(bytes) => yield Ok(bytes),
                                    Err(e) => {
                                        warn!(
                                            worker = %label,
                                            error = %e,
                                            "serialization failed"
                                        );
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(worker = %label, error = %e, "failed to parse inference request");
                            yield Err(RpcError::SerializationError(e));
                        }
                    }
                }
            }
        }
    }).await;

    // Start USER port server
    let user_quic = user_server.bind()?;
    let user_actual_addr = user_server.socket_addr.ok_or_else(|| anyhow::anyhow!("no user socket addr"))?;
    info!(worker = %worker_label, port = %user_actual_addr, "🎧 USER port listening (RPC endpoints)");

    let worker_label_user = worker_label.clone();
    tokio::spawn(async move {
        if let Err(e) = user_server.start(user_quic).await {
            error!(worker = %worker_label_user, error = %e, "user port server failed");
        }
    });

    // Start MANAGEMENT port server
    let mgmt_quic = mgmt_server.bind()?;
    let mgmt_actual_addr = mgmt_server.socket_addr.ok_or_else(|| anyhow::anyhow!("no mgmt socket addr"))?;
    info!(worker = %worker_label, port = %mgmt_actual_addr, "⚙️  MGMT port listening (health checks, registration)");

    let worker_label_mgmt = worker_label.clone();
    tokio::spawn(async move {
        if let Err(e) = mgmt_server.start(mgmt_quic).await {
            error!(worker = %worker_label_mgmt, error = %e, "mgmt port server failed");
        }
    });

    sleep(Duration::from_millis(500)).await;

    // Generate stable worker ID once
    let worker_id = uuid::Uuid::new_v4();
    info!(worker = %worker_label, worker.id = %worker_id, "🆔 worker ID assigned");
    
    // Connect once to director's MANAGEMENT port
    let director_mgmt_addr = SocketAddr::from_str(&director_mgmt_target)?;
    let register_interval = Duration::from_secs(5);
    
    let director_config = RpcConfig::new("../../certs/test_cert.pem", "0.0.0.0:0");
    let director_client = loop {
        match RpcClient::connect(director_mgmt_addr, director_config.clone()).await {
            Ok(client) => {
                info!(worker = %worker_label, "⚙️  MGMT: connected to director management port");
                break client;
            }
            Err(e) => {
                warn!(worker = %worker_label, error = %e, "⚙️  MGMT: failed to connect, retrying in 5s");
                sleep(register_interval).await;
            }
        }
    };
    
    // Registration loop - reuse the same connection
    loop {
        let available = *is_available.read().await;
        
        let register_req = RegisterWorkerRequest {
            worker: WorkerInfo {
                worker_id,
                label: worker_label.clone(),
                user_addr: user_addr.clone(),
                management_addr: mgmt_addr.clone(),
                capacity: 10,
                available,
            },
        };

        let req_bytes = match bincode::serialize(&register_req) {
            Ok(b) => b,
            Err(e) => {
                error!(worker = %worker_label, error = %e, "failed to serialize registration");
                sleep(register_interval).await;
                continue;
            }
        };

        match director_client.call("register_worker", req_bytes).await {
            Ok(response_bytes) => {
                match bincode::deserialize::<RegisterWorkerResponse>(&response_bytes) {
                    Ok(response) if response.success => {
                        tracing::debug!(
                            worker = %worker_label,
                            worker.id = %response.worker_id,
                            "⚙️  MGMT: heartbeat sent to director"
                        );
                    }
                    Ok(_) => {
                        warn!(worker = %worker_label, "⚙️  MGMT: registration rejected by director");
                    }
                    Err(e) => {
                        error!(worker = %worker_label, error = %e, "⚙️  MGMT: failed to parse response");
                    }
                }
            }
            Err(e) => {
                error!(worker = %worker_label, error = %e, "⚙️  MGMT: failed to send heartbeat - connection may be lost");
            }
        }

        sleep(register_interval).await;
    }
}
