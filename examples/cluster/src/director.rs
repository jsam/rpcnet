use anyhow::Result;
use async_trait::async_trait;
use cluster_example::generated::directorregistry::*;
use rpcnet::cluster::{
    ClusterConfig, LoadBalancingStrategy, WorkerRegistry, NodeRegistry,
    gossip::NodeState,
    incarnation::{Incarnation, NodeStatus},
};
use rpcnet::{RpcConfig, RpcError, RpcServer};
use s2n_quic::Client as QuicClient;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::info;
use uuid::Uuid;

struct DirectorHandler {
    worker_registry: Arc<WorkerRegistry>,
}

#[async_trait]
impl DirectorRegistryHandler for DirectorHandler {
    async fn get_worker(
        &self,
        request: GetWorkerRequest,
    ) -> Result<GetWorkerResponse, DirectorError> {
        let connection_id = request.connection_id.unwrap_or_else(|| {
            format!("conn-{}", Uuid::new_v4())
        });

        info!(
            connection.id = %connection_id,
            "📨 client requesting worker assignment"
        );

        match self.worker_registry.select_worker(None).await {
            Some(worker) => {
                worker.increment_connections();
                let worker_label = worker.node_id.as_str().to_string();
                let worker_addr = worker.addr.to_string();
                
                let response = GetWorkerResponse {
                    success: true,
                    worker_addr: Some(worker_addr.clone()),
                    worker_label: Some(worker_label.clone()),
                    connection_id: connection_id.clone(),
                    message: None,
                };
                
                info!(
                    connection.id = %connection_id,
                    worker = %worker_label,
                    worker.addr = %worker_addr,
                    connections = worker.connection_count(),
                    "✅ assigned worker to client"
                );
                
                Ok(response)
            }
            None => {
                info!(
                    connection.id = %connection_id,
                    "⚠️  no workers available"
                );
                
                Ok(GetWorkerResponse {
                    success: false,
                    worker_addr: None,
                    worker_label: None,
                    connection_id: connection_id.clone(),
                    message: Some("No workers available".to_string()),
                })
            }
        }
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

    let addr: SocketAddr = env::var("DIRECTOR_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string())
        .parse()?;

    info!("🎯 Starting Director at {}", addr);

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

    let mut server = RpcServer::new(config.clone());

    let srv = server.bind()?;

    let quic_client = Arc::new(
        QuicClient::builder()
            .with_tls(cert_path)?
            .with_io("0.0.0.0:0")?
            .start()?,
    );

    let cluster_config = ClusterConfig::default();
    server
        .enable_cluster(cluster_config.clone(), vec![], quic_client.clone())
        .await?;

    let cluster = server.cluster().await.expect("Cluster should be enabled");
    
    let director_node_id = cluster.node_id().clone();
    let director_status = NodeStatus {
        node_id: director_node_id.clone(),
        addr,
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        last_seen: std::time::Instant::now(),
        tags: {
            let mut tags = HashMap::new();
            tags.insert("role".to_string(), "director".to_string());
            tags
        },
    };
    cluster.registry().insert(director_status);
    info!("✅ Director registered itself in cluster");
    
    info!("✅ Cluster enabled - Director is now discoverable");

    let worker_registry = Arc::new(WorkerRegistry::new(
        cluster.clone(),
        LoadBalancingStrategy::LeastConnections,
    ));
    
    worker_registry.start().await;

    info!("🔄 Load balancing strategy: LeastConnections");

    let get_worker_registry = worker_registry.clone();
    server
        .register("DirectorRegistry.get_worker", move |params: Vec<u8>| {
            let registry = get_worker_registry.clone();
            async move {
                match bincode::deserialize::<GetWorkerRequest>(&params) {
                    Ok(request) => {
                        let connection_id = request.connection_id.unwrap_or_else(|| {
                            format!("conn-{}", Uuid::new_v4())
                        });

                        info!(
                            connection.id = %connection_id,
                            "📨 client requesting worker assignment"
                        );

                        match registry.select_worker(None).await {
                            Some(worker) => {
                                worker.increment_connections();
                                let worker_label = worker.node_id.as_str().to_string();
                                let worker_addr = worker.addr.to_string();
                                
                                let response = GetWorkerResponse {
                                    success: true,
                                    worker_addr: Some(worker_addr.clone()),
                                    worker_label: Some(worker_label.clone()),
                                    connection_id: connection_id.clone(),
                                    message: None,
                                };
                                
                                info!(
                                    connection.id = %connection_id,
                                    worker = %worker_label,
                                    worker.addr = %worker_addr,
                                    connections = worker.connection_count(),
                                    "✅ assigned worker to client"
                                );
                                
                                Ok(bincode::serialize(&response).map_err(|e| RpcError::SerializationError(e))?)
                            }
                            None => {
                                info!(
                                    connection.id = %connection_id,
                                    "⚠️  no workers available"
                                );
                                
                                let response = GetWorkerResponse {
                                    success: false,
                                    worker_addr: None,
                                    worker_label: None,
                                    connection_id: connection_id.clone(),
                                    message: Some("No workers available".to_string()),
                                };
                                
                                Ok(bincode::serialize(&response).map_err(|e| RpcError::SerializationError(e))?)
                            }
                        }
                    }
                    Err(e) => {
                        Err(RpcError::SerializationError(e))
                    }
                }
            }
        })
        .await;

    let stats_registry = worker_registry.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let count = stats_registry.worker_count().await;
            let workers = stats_registry.all_workers().await;
            
            if count > 0 {
                info!("📊 Worker pool status: {} workers available", count);
                for worker in workers {
                    info!(
                        "   - {} at {} ({} connections)",
                        worker.node_id.as_str(),
                        worker.addr,
                        worker.connection_count()
                    );
                }
            } else {
                info!("⚠️  No workers available");
            }
        }
    });


    info!("🚀 Director ready - listening on {}", addr);
    server.start(srv).await?;

    Ok(())
}
