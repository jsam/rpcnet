use anyhow::Result;
use connection_swap::protocol::*;
use rpcnet::{RpcClient, RpcConfig};
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn, error};
use uuid::Uuid;
use futures::StreamExt;
use tokio::time::{Duration, interval};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let director_target = env::var("CONNECTION_SWAP_DIRECTOR_TARGET")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string());

    info!(director = %director_target, "📡 starting client");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let prompt = format!("prompt-{}", Uuid::new_v4());
    let mut connection_id: Option<String> = None;
    let mut total_tokens = 0u64;

    loop {
        info!("🔍 asking director for worker assignment");
        
        let director_addr = SocketAddr::from_str(&director_target)?;
        let director_config = RpcConfig::new("../../certs/test_cert.pem", "0.0.0.0:0");
        
        let director_client = RpcClient::connect(director_addr, director_config).await?;
        info!("✅ connected to director");

        let get_worker_req = GetWorkerRequest {
            connection_id: connection_id.clone(),
            prompt: prompt.clone(),
        };

        let req_bytes = bincode::serialize(&get_worker_req)?;
        
        match director_client.call("get_worker", req_bytes).await {
            Ok(response_bytes) => {
                match bincode::deserialize::<GetWorkerResponse>(&response_bytes) {
                    Ok(response) if response.success => {
                        let worker_addr_str = response.worker_addr.unwrap();
                        let worker_label = response.worker_label.unwrap();
                        connection_id = Some(response.connection_id.clone());

                        info!(
                            connection.id = %response.connection_id,
                            worker = %worker_label,
                            worker.addr = %worker_addr_str,
                            "🔀 director assigned worker - establishing direct connection"
                        );

                        let worker_addr = SocketAddr::from_str(&worker_addr_str)?;
                        let worker_config = RpcConfig::new("../../certs/test_cert.pem", "0.0.0.0:0");
                        
                        match RpcClient::connect(worker_addr, worker_config).await {
                            Ok(worker_client) => {
                                info!(
                                    connection.id = %response.connection_id,
                                    worker = %worker_label,
                                    "✅ direct connection established to worker"
                                );

                                let worker_client = Arc::new(worker_client);
                                let heartbeat_failed = Arc::new(AtomicBool::new(false));
                                let heartbeat_failed_clone = heartbeat_failed.clone();
                                let worker_client_clone = worker_client.clone();
                                let cid = response.connection_id.clone();
                                let wl = worker_label.clone();

                                let heartbeat_handle = tokio::spawn(async move {
                                    let mut heartbeat_interval = interval(Duration::from_secs(5));
                                    heartbeat_interval.tick().await;
                                    
                                    let mut consecutive_failures = 0;
                                    
                                    loop {
                                        heartbeat_interval.tick().await;
                                        
                                        let heartbeat_req = HeartbeatRequest {
                                            connection_id: cid.clone(),
                                        };
                                        
                                        match bincode::serialize(&heartbeat_req) {
                                            Ok(req_bytes) => {
                                                match tokio::time::timeout(
                                                    Duration::from_secs(3),
                                                    worker_client_clone.call("heartbeat", req_bytes)
                                                ).await {
                                                    Ok(Ok(response_bytes)) => {
                                                        match bincode::deserialize::<HeartbeatResponse>(&response_bytes) {
                                                            Ok(resp) if resp.alive => {
                                                                consecutive_failures = 0;
                                                                info!(
                                                                    connection.id = %cid,
                                                                    worker = %wl,
                                                                    "💓 heartbeat ok"
                                                                );
                                                            }
                                                            Ok(_) => {
                                                                consecutive_failures += 1;
                                                                warn!(
                                                                    connection.id = %cid,
                                                                    worker = %wl,
                                                                    failures = consecutive_failures,
                                                                    "💔 heartbeat returned not alive"
                                                                );
                                                            }
                                                            Err(e) => {
                                                                consecutive_failures += 1;
                                                                warn!(
                                                                    connection.id = %cid,
                                                                    worker = %wl,
                                                                    error = %e,
                                                                    failures = consecutive_failures,
                                                                    "💔 heartbeat deserialize failed"
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Ok(Err(e)) => {
                                                        consecutive_failures += 1;
                                                        warn!(
                                                            connection.id = %cid,
                                                            worker = %wl,
                                                            error = %e,
                                                            failures = consecutive_failures,
                                                            "💔 heartbeat call failed"
                                                        );
                                                    }
                                                    Err(_) => {
                                                        consecutive_failures += 1;
                                                        warn!(
                                                            connection.id = %cid,
                                                            worker = %wl,
                                                            failures = consecutive_failures,
                                                            "💔 heartbeat timeout"
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!(error = %e, "failed to serialize heartbeat request");
                                            }
                                        }
                                        
                                        if consecutive_failures >= 2 {
                                            error!(
                                                connection.id = %cid,
                                                worker = %wl,
                                                failures = consecutive_failures,
                                                "💀 heartbeat failed {} times - marking worker as failed",
                                                consecutive_failures
                                            );
                                            heartbeat_failed_clone.store(true, Ordering::SeqCst);
                                            break;
                                        }
                                    }
                                });

                                let inference_req = InferenceRequest {
                                    connection_id: response.connection_id.clone(),
                                    prompt: prompt.clone(),
                                };

                                let inference_bytes = bincode::serialize(&inference_req)?;
                                
                                info!(
                                    connection.id = %response.connection_id,
                                    worker = %worker_label,
                                    "📤 creating request stream with {} bytes",
                                    inference_bytes.len()
                                );
                                
                                let request_stream = futures::stream::iter(vec![inference_bytes]);
                                
                                info!(
                                    connection.id = %response.connection_id,
                                    worker = %worker_label,
                                    "🔌 calling call_streaming on worker"
                                );
                                
                                match worker_client.call_streaming("generate", request_stream).await {
                                    Ok(stream) => {
                                        info!(
                                            connection.id = %response.connection_id,
                                            worker = %worker_label,
                                            "🌊 stream opened successfully, starting to consume responses"
                                        );

                                        let mut stream = Box::pin(stream);
                                        let mut worker_failed = false;

                                        while let Some(response_result) = stream.next().await {
                                            if heartbeat_failed.load(Ordering::SeqCst) {
                                                error!(
                                                    connection.id = %connection_id.as_ref().unwrap(),
                                                    worker = %worker_label,
                                                    "💀 heartbeat detected failure - aborting stream"
                                                );
                                                worker_failed = true;
                                                break;
                                            }
                                            match response_result {
                                                Ok(response_bytes) => {
                                                    match bincode::deserialize::<InferenceResponse>(&response_bytes) {
                                                        Ok(response) => {
                                                            match response {
                                                                InferenceResponse::Connected { worker: w, connection_id: cid } => {
                                                                    info!(
                                                                        connection.id = %cid,
                                                                        worker = %w,
                                                                        "🔄 worker confirmed connection"
                                                                    );
                                                                }
                                                                InferenceResponse::Token { text, sequence } => {
                                                                    total_tokens += 1;
                                                                    
                                                                    info!(
                                                                        connection.id = %connection_id.as_ref().unwrap(),
                                                                        worker = %worker_label,
                                                                        sequence = sequence,
                                                                        text = %text,
                                                                        total = total_tokens,
                                                                        "📦 received token"
                                                                    );
                                                                }
                                                                InferenceResponse::Error { message } => {
                                                                    warn!(
                                                                        connection.id = %connection_id.as_ref().unwrap(),
                                                                        worker = %worker_label,
                                                                        error = %message,
                                                                        "⚠️  worker failed - will request new worker from director"
                                                                    );
                                                                    worker_failed = true;
                                                                    break;
                                                                }
                                                                InferenceResponse::Done => {
                                                                    info!(
                                                                        connection.id = %connection_id.as_ref().unwrap(),
                                                                        total_tokens = total_tokens,
                                                                        "✅ stream completed"
                                                                    );
                                                                    return Ok(());
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            warn!(error = %e, "failed to deserialize response");
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error!(
                                                        connection.id = ?connection_id,
                                                        worker = %worker_label,
                                                        error = %e,
                                                        "❌ stream error"
                                                    );
                                                    worker_failed = true;
                                                    break;
                                                }
                                            }
                                        }

                                        heartbeat_handle.abort();
                                        
                                        if worker_failed {
                                            info!("🔄 returning to director for new worker assignment");
                                            tokio::time::sleep(Duration::from_secs(1)).await;
                                            continue;
                                        }
                                    }
                                    Err(e) => {
                                        heartbeat_handle.abort();
                                        error!(error = %e, "failed to start streaming with worker");
                                        tokio::time::sleep(Duration::from_secs(1)).await;
                                        continue;
                                    }
                                }
                            }
                            Err(e) => {
                                error!(
                                    worker = %worker_label,
                                    error = %e,
                                    "❌ failed to connect to worker - will retry with director"
                                );
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                        }
                    }
                    Ok(response) => {
                        warn!(
                            message = ?response.message,
                            "⏳ no workers available - retrying in 2s"
                        );
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        error!(error = %e, "failed to deserialize director response");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "failed to call director");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }
    }
}
