use anyhow::Result;
use connection_swap::protocol::*;
use rpcnet::{RpcServer, RpcClient, RpcConfig};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::pin::Pin;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn, error};
use uuid::Uuid;
use futures::{Stream, StreamExt};

#[derive(Clone)]
struct WorkerPool {
    workers: Arc<RwLock<HashMap<Uuid, WorkerInfo>>>,
    next_worker_idx: Arc<RwLock<usize>>,
}

impl WorkerPool {
    fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            next_worker_idx: Arc::new(RwLock::new(0)),
        }
    }

    async fn register(&self, worker: WorkerInfo) -> Uuid {
        let worker_id = worker.worker_id;
        let mut workers = self.workers.write().await;
        
        if workers.contains_key(&worker_id) {
            let prev_available = workers.get(&worker_id).map(|w| w.available).unwrap_or(false);
            
            if prev_available != worker.available {
                if worker.available {
                    info!(
                        worker.label = %worker.label,
                        worker.id = %worker_id,
                        "🟢 worker now AVAILABLE (recovered from failure)"
                    );
                } else {
                    info!(
                        worker.label = %worker.label,
                        worker.id = %worker_id,
                        "🔴 worker now UNAVAILABLE (entering recovery mode)"
                    );
                }
            } else {
                tracing::debug!(
                    worker.label = %worker.label,
                    worker.available = %worker.available,
                    worker.id = %worker_id,
                    "🔄 worker re-registered (heartbeat)"
                );
            }
        } else {
            info!(
                worker.label = %worker.label,
                worker.user_addr = %worker.user_addr,
                worker.id = %worker_id,
                worker.available = %worker.available,
                "✅ worker registered (new)"
            );
        }
        
        workers.insert(worker_id, worker);
        worker_id
    }

    async fn get_next_worker(&self) -> Option<(Uuid, WorkerInfo)> {
        let workers = self.workers.read().await;
        
        // Filter to only available workers
        let available_workers: Vec<_> = workers.iter()
            .filter(|(_, info)| info.available)
            .collect();
        
        if available_workers.is_empty() {
            return None;
        }

        let mut idx = self.next_worker_idx.write().await;
        let (id, info) = available_workers[*idx % available_workers.len()];
        *idx += 1;

        Some((*id, info.clone()))
    }

    async fn check_worker_health(&self, worker_id: Uuid, worker_info: &WorkerInfo) -> bool {
        // Health checks go to MANAGEMENT port
        let mgmt_addr = match SocketAddr::from_str(&worker_info.management_addr) {
            Ok(a) => a,
            Err(_) => return false,
        };

        let config = RpcConfig::new("../../certs/test_cert.pem", "0.0.0.0:0");
        let client = match RpcClient::connect(mgmt_addr, config).await {
            Ok(c) => c,
            Err(_) => return false,
        };

        let request = HealthCheckRequest {
            timestamp: SystemTime::now(),
        };

        let request_bytes = match bincode::serialize(&request) {
            Ok(b) => b,
            Err(_) => return false,
        };

        match tokio::time::timeout(
            Duration::from_secs(2),
            client.call("health_check", request_bytes)
        ).await {
            Ok(Ok(response_bytes)) => {
                match bincode::deserialize::<HealthCheckResponse>(&response_bytes) {
                    Ok(response) => response.healthy,
                    Err(_) => false,
                }
            }
            _ => false,
        }
    }

    async fn remove_worker(&self, worker_id: Uuid) {
        let mut workers = self.workers.write().await;
        if let Some(worker) = workers.remove(&worker_id) {
            info!(
                worker.id = %worker_id,
                worker.label = %worker.label,
                worker.user_addr = %worker.user_addr,
                "❌ worker disconnected (health check failed)"
            );
        }
    }

    async fn start_health_check_loop(self) {
        loop {
            sleep(Duration::from_secs(10)).await;

            let workers: Vec<_> = {
                let workers = self.workers.read().await;
                workers.iter().map(|(id, info)| (*id, info.clone())).collect()
            };

            for (worker_id, worker_info) in workers {
                if !self.check_worker_health(worker_id, &worker_info).await {
                    self.remove_worker(worker_id).await;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let user_addr_str = env::var("CONNECTION_SWAP_DIRECTOR_USER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61000".to_string());
    let mgmt_addr_str = env::var("CONNECTION_SWAP_DIRECTOR_MGMT_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:61001".to_string());

    info!(
        user.port = %user_addr_str,
        mgmt.port = %mgmt_addr_str,
        "🚀 starting director with dual-port architecture"
    );

    let worker_pool = WorkerPool::new();

    // ═══════════════════════════════════════════════════════════════
    // USER PORT - Client connections (generate endpoint)
    // ═══════════════════════════════════════════════════════════════
    let user_config = RpcConfig::new("../../certs/test_cert.pem", user_addr_str.clone())
        .with_key_path("../../certs/test_key.pem");
    let mut user_server = RpcServer::new(user_config);

    // ═══════════════════════════════════════════════════════════════
    // MANAGEMENT PORT - Worker registration and health checks
    // ═══════════════════════════════════════════════════════════════
    let mgmt_config = RpcConfig::new("../../certs/test_cert.pem", mgmt_addr_str.clone())
        .with_key_path("../../certs/test_key.pem");
    let mut mgmt_server = RpcServer::new(mgmt_config);

    // Worker registration on MANAGEMENT port
    let pool_clone = worker_pool.clone();
    mgmt_server.register("register_worker", move |params: Vec<u8>| {
        let pool = pool_clone.clone();
        async move {
            match bincode::deserialize::<RegisterWorkerRequest>(&params) {
                Ok(req) => {
                    let worker_id = pool.register(req.worker).await;
                    let response = RegisterWorkerResponse {
                        success: true,
                        worker_id,
                    };
                    bincode::serialize(&response).map_err(|e| {
                        rpcnet::RpcError::SerializationError(e)
                    })
                }
                Err(e) => Err(rpcnet::RpcError::SerializationError(e)),
            }
        }
    }).await;

    // Client worker assignment endpoint on USER port
    let pool_for_assignment = worker_pool.clone();
    user_server.register("get_worker", move |params: Vec<u8>| {
        let pool = pool_for_assignment.clone();
        async move {
            match bincode::deserialize::<GetWorkerRequest>(&params) {
                Ok(req) => {
                    let connection_id = req.connection_id.unwrap_or_else(|| format!("conn-{}", Uuid::new_v4()));
                    
                    match pool.get_next_worker().await {
                        Some((_worker_id, worker_info)) => {
                            info!(
                                connection.id = %connection_id,
                                worker.label = %worker_info.label,
                                worker.user_addr = %worker_info.user_addr,
                                prompt = %req.prompt,
                                "🔄 assigning worker to client"
                            );
                            
                            let response = GetWorkerResponse {
                                success: true,
                                worker_addr: Some(worker_info.user_addr.clone()),
                                worker_label: Some(worker_info.label.clone()),
                                connection_id,
                                message: None,
                            };
                            bincode::serialize(&response).map_err(|e| {
                                rpcnet::RpcError::SerializationError(e)
                            })
                        }
                        None => {
                            warn!(
                                connection.id = %connection_id,
                                "⏳ no workers available"
                            );
                            
                            let response = GetWorkerResponse {
                                success: false,
                                worker_addr: None,
                                worker_label: None,
                                connection_id,
                                message: Some("no workers available".to_string()),
                            };
                            bincode::serialize(&response).map_err(|e| {
                                rpcnet::RpcError::SerializationError(e)
                            })
                        }
                    }
                }
                Err(e) => Err(rpcnet::RpcError::SerializationError(e)),
            }
        }
    }).await;

    // Note: No streaming endpoint on director anymore
    // Director only assigns workers via get_worker
    // Client connects directly to workers for streaming

    // Start USER port server
    let user_quic = user_server.bind()?;
    let user_actual_addr = user_server.socket_addr.ok_or_else(|| anyhow::anyhow!("no user socket addr"))?;
    info!(port = %user_actual_addr, "🎧 USER port listening (client connections)");

    tokio::spawn(async move {
        if let Err(e) = user_server.start(user_quic).await {
            error!(error = %e, "user port server failed");
        }
    });

    // Start MANAGEMENT port server
    let mgmt_quic = mgmt_server.bind()?;
    let mgmt_actual_addr = mgmt_server.socket_addr.ok_or_else(|| anyhow::anyhow!("no mgmt socket addr"))?;
    info!(port = %mgmt_actual_addr, "⚙️  MGMT port listening (worker registration, health checks)");

    tokio::spawn(async move {
        if let Err(e) = mgmt_server.start(mgmt_quic).await {
            error!(error = %e, "mgmt port server failed");
        }
    });

    // Start health check loop
    let health_check_pool = worker_pool.clone();
    tokio::spawn(async move {
        health_check_pool.start_health_check_loop().await;
    });

    info!("✅ Director ready - both ports active");

    // Keep main thread alive
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}
