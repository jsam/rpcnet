use crate::cluster::worker_registry::{LoadBalancingStrategy, WorkerRegistry};
use crate::{RpcClient, RpcConfig, RpcError};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ClusterClient {
    registry: Arc<WorkerRegistry>,
    config: RpcConfig,
    clients: Arc<tokio::sync::RwLock<HashMap<std::net::SocketAddr, Arc<RpcClient>>>>,
}

impl ClusterClient {
    pub fn new(registry: Arc<WorkerRegistry>, config: RpcConfig) -> Self {
        Self {
            registry,
            config,
            clients: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn call_worker(
        &self,
        method: &str,
        params: Vec<u8>,
        tag_filter: Option<&HashMap<String, String>>,
    ) -> Result<Vec<u8>, RpcError> {
        let worker = self
            .registry
            .select_worker(tag_filter)
            .await
            .ok_or_else(|| RpcError::ConnectionError("No available workers".to_string()))?;

        let client = self.get_or_create_client(worker.addr).await?;

        worker.increment_connections();
        let result = client.call(method, params).await;
        worker.decrement_connections();

        result
    }

    async fn get_or_create_client(
        &self,
        addr: std::net::SocketAddr,
    ) -> Result<Arc<RpcClient>, RpcError> {
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&addr) {
                return Ok(client.clone());
            }
        }

        let client = Arc::new(RpcClient::connect(addr, self.config.clone()).await?);

        let mut clients = self.clients.write().await;
        clients.insert(addr, client.clone());

        Ok(client)
    }

    pub async fn call_all_workers(
        &self,
        method: &str,
        params: Vec<u8>,
        tag_filter: Option<&HashMap<String, String>>,
    ) -> Vec<Result<Vec<u8>, RpcError>> {
        let workers = if let Some(filter) = tag_filter {
            let mut filtered = Vec::new();
            for worker in self.registry.all_workers().await {
                if filter
                    .iter()
                    .all(|(k, v)| worker.tags.get(k).map(|val| val == v).unwrap_or(false))
                {
                    filtered.push(worker);
                }
            }
            filtered
        } else {
            self.registry.all_workers().await
        };

        let mut tasks = Vec::new();

        for worker in workers {
            let addr = worker.addr;
            let method = method.to_string();
            let params = params.clone();
            let clients = self.clients.clone();
            let config = self.config.clone();

            let task = tokio::spawn(async move {
                let client = {
                    let clients_read = clients.read().await;
                    if let Some(client) = clients_read.get(&addr) {
                        client.clone()
                    } else {
                        drop(clients_read);
                        let new_client = Arc::new(RpcClient::connect(addr, config).await?);
                        let mut clients_write = clients.write().await;
                        clients_write.insert(addr, new_client.clone());
                        new_client
                    }
                };

                worker.increment_connections();
                let result = client.call(&method, params).await;
                worker.decrement_connections();
                result
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(RpcError::ConnectionError(e.to_string()))),
            }
        }

        results
    }

    pub fn strategy(&self) -> LoadBalancingStrategy {
        self.registry.strategy()
    }

    pub async fn worker_count(&self) -> usize {
        self.registry.worker_count().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{ClusterConfig, ClusterMembership, LoadBalancingStrategy, WorkerRegistry};
    use s2n_quic::Client as QuicClient;
    use std::net::SocketAddr;
    use std::path::Path;

    async fn create_test_client() -> Arc<QuicClient> {
        let cert_path = Path::new("certs/test_cert.pem");
        let client = QuicClient::builder()
            .with_tls(cert_path)
            .unwrap()
            .with_io("0.0.0.0:0")
            .unwrap()
            .start()
            .unwrap();

        Arc::new(client)
    }

    async fn create_test_cluster_client(
        strategy: LoadBalancingStrategy,
    ) -> (ClusterClient, Arc<ClusterMembership>) {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let quic_client = create_test_client().await;

        let cluster = Arc::new(
            ClusterMembership::new(addr, config, quic_client)
                .await
                .unwrap(),
        );
        let registry = Arc::new(WorkerRegistry::new(cluster.clone(), strategy));

        let rpc_config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0");
        let cluster_client = ClusterClient::new(registry, rpc_config);

        (cluster_client, cluster)
    }

    #[tokio::test]
    async fn test_cluster_client_creation() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::RoundRobin).await;

        assert_eq!(cluster_client.worker_count().await, 0);
        assert_eq!(cluster_client.strategy(), LoadBalancingStrategy::RoundRobin);
    }

    #[tokio::test]
    async fn test_cluster_client_with_random_strategy() {
        let (cluster_client, _) = create_test_cluster_client(LoadBalancingStrategy::Random).await;

        assert_eq!(cluster_client.worker_count().await, 0);
        assert_eq!(cluster_client.strategy(), LoadBalancingStrategy::Random);
    }

    #[tokio::test]
    async fn test_cluster_client_with_least_connections_strategy() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::LeastConnections).await;

        assert_eq!(cluster_client.worker_count().await, 0);
        assert_eq!(
            cluster_client.strategy(),
            LoadBalancingStrategy::LeastConnections
        );
    }

    #[tokio::test]
    async fn test_call_worker_no_workers_available() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::RoundRobin).await;

        let result = cluster_client
            .call_worker("test_method", vec![1, 2, 3], None)
            .await;

        assert!(result.is_err());
        match result {
            Err(RpcError::ConnectionError(msg)) => {
                assert_eq!(msg, "No available workers");
            }
            _ => panic!("Expected ConnectionError with 'No available workers'"),
        }
    }

    #[tokio::test]
    async fn test_call_all_workers_empty() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::RoundRobin).await;

        let results = cluster_client
            .call_all_workers("test_method", vec![1, 2, 3], None)
            .await;

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_cluster_client_new_creates_empty_clients_map() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let quic_client = create_test_client().await;

        let cluster = Arc::new(
            ClusterMembership::new(addr, config, quic_client)
                .await
                .unwrap(),
        );
        let registry = Arc::new(WorkerRegistry::new(
            cluster,
            LoadBalancingStrategy::RoundRobin,
        ));

        let rpc_config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0");
        let cluster_client = ClusterClient::new(registry.clone(), rpc_config);

        // Verify internal state is initialized
        assert_eq!(cluster_client.worker_count().await, 0);
        let clients_lock = cluster_client.clients.read().await;
        assert_eq!(clients_lock.len(), 0);
    }

    #[tokio::test]
    async fn test_strategy_returns_correct_value() {
        let strategies = vec![
            LoadBalancingStrategy::RoundRobin,
            LoadBalancingStrategy::Random,
            LoadBalancingStrategy::LeastConnections,
        ];

        for strategy in strategies {
            let (cluster_client, _) = create_test_cluster_client(strategy).await;
            assert_eq!(cluster_client.strategy(), strategy);
        }
    }

    #[tokio::test]
    async fn test_worker_count_returns_zero_initially() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::RoundRobin).await;
        let count = cluster_client.worker_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_call_all_workers_with_filter_empty() {
        let (cluster_client, _) =
            create_test_cluster_client(LoadBalancingStrategy::RoundRobin).await;

        let mut filter = HashMap::new();
        filter.insert("role".to_string(), "gpu".to_string());

        let results = cluster_client
            .call_all_workers("test_method", vec![1, 2, 3], Some(&filter))
            .await;

        assert_eq!(results.len(), 0);
    }
}
