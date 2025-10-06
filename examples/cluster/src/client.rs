use anyhow::Result;
use cluster_example::generated::directorregistry::*;
use cluster_example::generated::inference::*;
use futures::StreamExt;
use rpcnet::RpcConfig;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cluster_example=info".parse()?),
        )
        .init();

    let director_addr: SocketAddr = env::var("DIRECTOR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string())
        .parse()?;

    info!("📡 Starting Client - connecting to director at {}", director_addr);

    let cert_path = if Path::new("../../certs/test_cert.pem").exists() {
        Path::new("../../certs/test_cert.pem")
    } else if Path::new("certs/test_cert.pem").exists() {
        Path::new("certs/test_cert.pem")
    } else {
        panic!("Cannot find test_cert.pem in certs/ or ../../certs/");
    };
    
    let key_path = if Path::new("../../certs/test_key.pem").exists() {
        Path::new("../../certs/test_key.pem")
    } else if Path::new("certs/test_key.pem").exists() {
        Path::new("certs/test_key.pem")
    } else {
        panic!("Cannot find test_key.pem in certs/ or ../../certs/");
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    let prompt = format!("prompt-{}", Uuid::new_v4());
    let mut connection_id: Option<String> = None;
    let mut total_tokens = 0u64;

    loop {
        info!("🔍 asking director for worker assignment");
        
        let director_config = RpcConfig::new(cert_path, "0.0.0.0:0")
            .with_key_path(key_path)
            .with_server_name("localhost")
            .with_default_stream_timeout(Duration::from_secs(3));
        
        let director_client = match DirectorRegistryClient::connect(director_addr, director_config).await {
            Ok(client) => client,
            Err(e) => {
                error!("❌ failed to connect to director: {}", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        info!("✅ connected to director");

        let get_worker_req = GetWorkerRequest {
            connection_id: connection_id.clone(),
            prompt: prompt.clone(),
        };

        match director_client.get_worker(get_worker_req).await {
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
                let worker_config = RpcConfig::new(cert_path, "0.0.0.0:0")
                    .with_key_path(key_path)
                    .with_server_name("localhost")
                    .with_default_stream_timeout(Duration::from_secs(3));
                
                match InferenceClient::connect(worker_addr, worker_config).await {
                    Ok(worker_client) => {
                        info!(
                            connection.id = %response.connection_id,
                            worker = %worker_label,
                            "✅ direct connection established to worker"
                        );

                        info!(
                            connection.id = %response.connection_id,
                            worker = %worker_label,
                            "📤 creating bidirectional request stream"
                        );
                        
                        let conn_id = response.connection_id.clone();
                        let req_prompt = prompt.clone();
                        
                        let mut bidir_stream = rpcnet::streaming::BidirectionalStream::with_task(10, {
                            let conn_id = conn_id.clone();
                            move |sender| async move {
                                for i in 0..100 {
                                    let req = InferenceRequest {
                                        connection_id: conn_id.clone(),
                                        prompt: format!("{}-chunk-{}", req_prompt, i),
                                    };
                                    info!(
                                        connection.id = %conn_id,
                                        chunk = i,
                                        "📤 client sending request chunk"
                                    );
                                    if sender.send(req).await.is_err() {
                                        info!(connection.id = %conn_id, "📤 request stream closed by receiver");
                                        break;
                                    }
                                    tokio::time::sleep(Duration::from_millis(800)).await;
                                }
                            }
                        });
                        
                        info!(
                            connection.id = %response.connection_id,
                            worker = %worker_label,
                            "🔌 calling generate on worker"
                        );
                        
                        let stream_to_send = bidir_stream.into_stream();
                        
                        match worker_client.generate(stream_to_send).await {
                            Ok(stream) => {
                                info!(
                                    connection.id = %response.connection_id,
                                    worker = %worker_label,
                                    "🌊 stream opened successfully, starting to consume responses"
                                );

                                let mut stream = Box::pin(stream);
                                let mut worker_failed = false;

                                loop {
                                    let result = match stream.next().await {
                                        Some(r) => r,
                                        None => {
                                            info!(
                                                connection.id = %connection_id.as_ref().unwrap(),
                                                worker = %worker_label,
                                                "Stream ended normally"
                                            );
                                            break;
                                        }
                                    };
                                    
                                    let response = match result {
                                        Ok(resp) => resp,
                                        Err(e) => {
                                            error!(
                                                connection.id = %connection_id.as_ref().unwrap(),
                                                worker = %worker_label,
                                                error = ?e,
                                                "❌ stream error - will request new worker from director"
                                            );
                                            worker_failed = true;
                                            break;
                                        }
                                    };
                                    
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
                                
                                drop(stream);
                                
                                if worker_failed {
                                    info!("🔄 returning to director for new worker assignment");
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    continue;
                                }
                            }
                            Err(e) => {
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
                error!(error = %e, "failed to call director");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }
    }
}
