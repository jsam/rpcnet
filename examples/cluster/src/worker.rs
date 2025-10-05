use anyhow::Result;
use async_trait::async_trait;
use cluster_example::generated::inference::*;
use futures::Stream;
use futures::StreamExt;
use rpcnet::cluster::ClusterConfig;
use rpcnet::{RpcConfig, RpcError};
use s2n_quic::Client as QuicClient;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use rand::Rng;

struct WorkerHandler {
    worker_label: String,
    is_failed: Arc<AtomicBool>,
    failure_enabled: bool,
}

#[async_trait]
impl InferenceHandler for WorkerHandler {
    async fn generate(
        &self,
        request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<InferenceResponse, InferenceError>> + Send>>, InferenceError> {
        let name = self.worker_label.clone();
        let is_failed = self.is_failed.clone();
        
        if is_failed.load(Ordering::SeqCst) {
            error!("🚫 [{}] Rejecting request - worker is in failed state", name);
            return Err(InferenceError::WorkerFailed(format!("Worker {} is currently failed", name)));
        }
        
        info!("🎬 [{}] Streaming handler invoked", name);
        
        let failure_enabled = self.failure_enabled;
        let response_stream = async_stream::stream! {
            let mut request_stream = Box::pin(request);
            let mut conn_id = String::new();
            let mut first_request = true;
            
            while let Some(req) = request_stream.next().await {
                if is_failed.load(Ordering::SeqCst) {
                    yield Ok(InferenceResponse::Error {
                        message: format!("Worker {} failed during streaming", name),
                    });
                    return;
                }
                
                if first_request {
                    conn_id = req.connection_id.clone();
                    
                    if failure_enabled && rand::random::<f32>() < 0.7 {
                        let delay = rand::thread_rng().gen_range(5..15);
                        let recovery_time = rand::thread_rng().gen_range(10..30);
                        warn!("⚠️  [{}] Will simulate failure in {} responses, recovery in {}s...", name, delay, recovery_time);
                        tokio::spawn({
                            let is_failed = is_failed.clone();
                            let name = name.clone();
                            async move {
                                sleep(Duration::from_millis(delay as u64 * 200)).await;
                                is_failed.store(true, Ordering::SeqCst);
                                error!("💥 [{}] Worker failed!", name);
                                sleep(Duration::from_secs(recovery_time)).await;
                                info!("🔄 [{}] Worker recovering...", name);
                                is_failed.store(false, Ordering::SeqCst);
                                info!("✅ [{}] Worker recovered and ready to serve!", name);
                            }
                        });
                    }
                    info!(
                        connection.id = %conn_id,
                        worker = %name,
                        prompt = %req.prompt,
                        "✅ received first request - establishing bidirectional stream"
                    );
                    
                    yield Ok(InferenceResponse::Connected {
                        worker: name.clone(),
                        connection_id: conn_id.clone(),
                    });
                    first_request = false;
                } else {
                    info!(
                        connection.id = %conn_id,
                        worker = %name,
                        prompt = %req.prompt,
                        "📥 worker received request chunk"
                    );
                }
                
                sleep(Duration::from_millis(200)).await;
                
                yield Ok(InferenceResponse::Token {
                    text: format!("[{}] processed: {}", name, req.prompt),
                    sequence: 0,
                });
            }
            
            info!(
                connection.id = %conn_id,
                worker = %name,
                "✅ client closed request stream - ending response stream"
            );
        };
        
        Ok(Box::pin(response_stream))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cluster_example=info".parse()?),
        )
        .init();

    let worker_label = env::var("WORKER_LABEL").unwrap_or_else(|_| "worker-1".to_string());
    let addr: SocketAddr = env::var("WORKER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:62001".to_string())
        .parse()?;
    let director_addr: SocketAddr = env::var("DIRECTOR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string())
        .parse()?;

    info!("👷 Starting Worker '{}' at {}", worker_label, addr);

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
    
    info!("📁 Loading certificates from {:?} and {:?}", cert_path, key_path);

    let config = RpcConfig::new(cert_path, addr.to_string())
        .with_key_path(key_path)
        .with_server_name("localhost");

    let worker_failure_enabled = env::var("WORKER_FAILURE_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let is_failed = Arc::new(AtomicBool::new(false));
    
    let handler = WorkerHandler {
        worker_label: worker_label.clone(),
        is_failed: is_failed.clone(),
        failure_enabled: worker_failure_enabled,
    };
    
    let mut server = InferenceServer::new(handler, config.clone());
    server.register_all().await;

    info!("🔌 Binding server to {}...", addr);
    let srv = server.rpc_server.bind()?;
    info!("✅ Server bound successfully to {}", addr);

    let quic_client = Arc::new(
        QuicClient::builder()
            .with_tls(cert_path)?
            .with_io("0.0.0.0:0")?
            .start()?,
    );

    info!("🌐 Enabling cluster, connecting to director at {}...", director_addr);
    let cluster_config = ClusterConfig::default();
    server.rpc_server
        .enable_cluster(cluster_config, vec![director_addr], quic_client)
        .await?;
    info!("✅ Cluster enabled, connected to director");

    let cluster = server.rpc_server.cluster().await.expect("Cluster should be enabled");

    info!("🏷️  Tagging worker with role=worker and label={}...", worker_label);
    cluster.update_tag("role".to_string(), "worker".to_string()).await;
    cluster.update_tag("label".to_string(), worker_label.clone()).await;
    
    info!(
        "✅ Worker '{}' joined cluster with role=worker, label={}",
        worker_label, worker_label
    );

    let event_cluster = cluster.clone();
    let event_label = worker_label.clone();
    tokio::spawn(async move {
        let mut events = event_cluster.subscribe();
        while let Ok(event) = events.recv().await {
            info!("[{}] Cluster event: {:?}", event_label, event);
        }
    });
    
    if worker_failure_enabled {
        info!("⚠️  [{}] Worker failure simulation enabled - will fail randomly during request processing", worker_label);
    }

    info!("🚀 Worker '{}' is running and ready to handle requests", worker_label);
    server.rpc_server.start(srv).await?;
    
    Ok(())
}
