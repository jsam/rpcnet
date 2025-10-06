use crate::cluster::gossip::NodeId;
use crate::cluster::incarnation::NodeStatus;
use crate::cluster::membership::ClusterMembership;
use crate::cluster::node_registry::NodeRegistry;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    Random,
    LeastConnections,
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub node_id: NodeId,
    pub addr: SocketAddr,
    pub tags: HashMap<String, String>,
    connections: Arc<AtomicUsize>,
}

impl WorkerInfo {
    fn from_node_status(status: NodeStatus) -> Self {
        Self {
            node_id: status.node_id,
            addr: status.addr,
            tags: status.tags,
            connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn increment_connections(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_connections(&self) {
        self.connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }
}

pub struct WorkerRegistry {
    workers: Arc<RwLock<HashMap<NodeId, WorkerInfo>>>,
    strategy: LoadBalancingStrategy,
    round_robin_counter: Arc<AtomicUsize>,
    cluster: Arc<ClusterMembership>,
}

impl WorkerRegistry {
    pub fn new(cluster: Arc<ClusterMembership>, strategy: LoadBalancingStrategy) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            strategy,
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
            cluster,
        }
    }

    pub async fn start(&self) {
        let workers = self.workers.clone();
        let cluster = self.cluster.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

                let all_nodes = cluster.registry().all_nodes();
                let all_workers: Vec<_> = all_nodes
                    .into_iter()
                    .filter(|node| {
                        node.state == crate::cluster::gossip::NodeState::Alive
                            && node.tags.get("role").map(|v| v.as_str()) == Some("worker")
                            && node.tags.get("status").map(|v| v.as_str()) != Some("failed")
                    })
                    .collect();
                
                let mut workers_guard = workers.write().await;
                
                let current_node_ids: std::collections::HashSet<_> = 
                    all_workers.iter().map(|n| n.node_id.clone()).collect();
                
                workers_guard.retain(|node_id, _| current_node_ids.contains(node_id));
                
                for node_status in all_workers {
                    workers_guard.entry(node_status.node_id.clone()).or_insert_with(|| {
                        WorkerInfo::from_node_status(node_status)
                    });
                }
            }
        });
    }

    pub async fn select_worker(&self, tag_filter: Option<&HashMap<String, String>>) -> Option<WorkerInfo> {
        let workers = self.workers.read().await;
        
        let candidates: Vec<_> = workers
            .values()
            .filter(|w| {
                if let Some(filter) = tag_filter {
                    filter.iter().all(|(k, v)| w.tags.get(k).map(|val| val == v).unwrap_or(false))
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let idx = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % candidates.len();
                Some(candidates[idx].clone())
            }
            LoadBalancingStrategy::Random => {
                use rand::Rng;
                let idx = rand::thread_rng().gen_range(0..candidates.len());
                Some(candidates[idx].clone())
            }
            LoadBalancingStrategy::LeastConnections => {
                candidates.into_iter().min_by_key(|w| w.connection_count())
            }
        }
    }

    pub async fn all_workers(&self) -> Vec<WorkerInfo> {
        self.workers.read().await.values().cloned().collect()
    }

    pub async fn workers_with_tag(&self, key: &str, value: &str) -> Vec<WorkerInfo> {
        self.workers
            .read()
            .await
            .values()
            .filter(|w| w.tags.get(key).map(|v| v.as_str()) == Some(value))
            .cloned()
            .collect()
    }

    pub async fn worker_count(&self) -> usize {
        self.workers.read().await.len()
    }

    pub fn strategy(&self) -> LoadBalancingStrategy {
        self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{ClusterConfig, ClusterMembership};
    use s2n_quic::Client as QuicClient;
    use std::path::Path;
    use std::time::Duration;

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

    #[tokio::test]
    async fn test_worker_registry_creation() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let client = create_test_client().await;

        let cluster = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());
        let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::RoundRobin);

        assert_eq!(registry.strategy(), LoadBalancingStrategy::RoundRobin);
        assert_eq!(registry.worker_count().await, 0);
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        let client = create_test_client().await;

        let cluster = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());
        let registry = WorkerRegistry::new(cluster.clone(), LoadBalancingStrategy::RoundRobin);

        let worker1 = WorkerInfo {
            node_id: NodeId::new("worker-1"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            tags: HashMap::new(),
            connections: Arc::new(AtomicUsize::new(0)),
        };

        let worker2 = WorkerInfo {
            node_id: NodeId::new("worker-2"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            tags: HashMap::new(),
            connections: Arc::new(AtomicUsize::new(0)),
        };

        {
            let mut workers = registry.workers.write().await;
            workers.insert(worker1.node_id.clone(), worker1.clone());
            workers.insert(worker2.node_id.clone(), worker2.clone());
        }

        let selected1 = registry.select_worker(None).await;
        let selected2 = registry.select_worker(None).await;

        assert!(selected1.is_some());
        assert!(selected2.is_some());
        assert_ne!(selected1.unwrap().node_id, selected2.unwrap().node_id);
    }

    #[tokio::test]
    async fn test_least_connections_selection() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();
        let client = create_test_client().await;

        let cluster = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());
        let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::LeastConnections);

        let worker1 = WorkerInfo {
            node_id: NodeId::new("worker-1"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            tags: HashMap::new(),
            connections: Arc::new(AtomicUsize::new(5)),
        };

        let worker2 = WorkerInfo {
            node_id: NodeId::new("worker-2"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            tags: HashMap::new(),
            connections: Arc::new(AtomicUsize::new(2)),
        };

        {
            let mut workers = registry.workers.write().await;
            workers.insert(worker1.node_id.clone(), worker1);
            workers.insert(worker2.node_id.clone(), worker2.clone());
        }

        let selected = registry.select_worker(None).await;
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().node_id, NodeId::new("worker-2"));
    }

    #[tokio::test]
    async fn test_tag_filtering() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:9003".parse().unwrap();
        let client = create_test_client().await;

        let cluster = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());
        let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::RoundRobin);

        let mut tags1 = HashMap::new();
        tags1.insert("role".to_string(), "compute".to_string());

        let mut tags2 = HashMap::new();
        tags2.insert("role".to_string(), "storage".to_string());

        let worker1 = WorkerInfo {
            node_id: NodeId::new("worker-1"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            tags: tags1,
            connections: Arc::new(AtomicUsize::new(0)),
        };

        let worker2 = WorkerInfo {
            node_id: NodeId::new("worker-2"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            tags: tags2,
            connections: Arc::new(AtomicUsize::new(0)),
        };

        {
            let mut workers = registry.workers.write().await;
            workers.insert(worker1.node_id.clone(), worker1.clone());
            workers.insert(worker2.node_id.clone(), worker2);
        }

        let mut filter = HashMap::new();
        filter.insert("role".to_string(), "compute".to_string());

        let selected = registry.select_worker(Some(&filter)).await;
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().node_id, NodeId::new("worker-1"));
    }

    #[tokio::test]
    async fn test_workers_with_tag() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:9004".parse().unwrap();
        let client = create_test_client().await;

        let cluster = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());
        let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::RoundRobin);

        let mut tags1 = HashMap::new();
        tags1.insert("zone".to_string(), "us-east".to_string());

        let mut tags2 = HashMap::new();
        tags2.insert("zone".to_string(), "us-east".to_string());

        let mut tags3 = HashMap::new();
        tags3.insert("zone".to_string(), "eu-west".to_string());

        let worker1 = WorkerInfo {
            node_id: NodeId::new("worker-1"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            tags: tags1,
            connections: Arc::new(AtomicUsize::new(0)),
        };

        let worker2 = WorkerInfo {
            node_id: NodeId::new("worker-2"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            tags: tags2,
            connections: Arc::new(AtomicUsize::new(0)),
        };

        let worker3 = WorkerInfo {
            node_id: NodeId::new("worker-3"),
            addr: "127.0.0.1:8003".parse().unwrap(),
            tags: tags3,
            connections: Arc::new(AtomicUsize::new(0)),
        };

        {
            let mut workers = registry.workers.write().await;
            workers.insert(worker1.node_id.clone(), worker1);
            workers.insert(worker2.node_id.clone(), worker2);
            workers.insert(worker3.node_id.clone(), worker3);
        }

        let us_east_workers = registry.workers_with_tag("zone", "us-east").await;
        assert_eq!(us_east_workers.len(), 2);
    }

    #[tokio::test]
    async fn test_worker_info_connections() {
        let worker = WorkerInfo {
            node_id: NodeId::new("test-worker"),
            addr: "127.0.0.1:8000".parse().unwrap(),
            tags: HashMap::new(),
            connections: Arc::new(AtomicUsize::new(0)),
        };

        assert_eq!(worker.connection_count(), 0);

        worker.increment_connections();
        assert_eq!(worker.connection_count(), 1);

        worker.increment_connections();
        assert_eq!(worker.connection_count(), 2);

        worker.decrement_connections();
        assert_eq!(worker.connection_count(), 1);
    }
}
