use crate::cluster::connection_pool::{ConnectionPool, ConnectionPoolImpl, PoolConfig};
use crate::cluster::events::{ClusterEvent, ClusterEventBroadcaster, ClusterEventReceiver};
use crate::cluster::gossip::{GossipConfig, GossipProtocol, GossipQueue, NodeId, NodeState, NodeUpdate, Priority, SwimProtocol};
use crate::cluster::health_checker::{HealthCheckConfig, HealthChecker};
use crate::cluster::incarnation::{Incarnation, NodeStatus};
use crate::cluster::node_registry::{NodeRegistry, SharedNodeRegistry};
use s2n_quic::Client as QuicClient;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub node_id: Option<NodeId>,
    pub gossip: GossipConfig,
    pub health: HealthCheckConfig,
    pub pool: PoolConfig,
    pub bootstrap_timeout: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            gossip: GossipConfig::default(),
            health: HealthCheckConfig::default(),
            pool: PoolConfig::default(),
            bootstrap_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClusterError {
    #[error("No seed nodes are reachable")]
    NoSeedsReachable,
    
    #[error("Bootstrap timeout after {0:?}")]
    BootstrapTimeout(Duration),
    
    #[error("Cluster already joined")]
    AlreadyJoined,
    
    #[error("Cluster not joined")]
    NotJoined,
    
    #[error("Failed to create QUIC client: {0}")]
    QuicClientError(String),
}

pub struct ClusterMembership {
    node_id: NodeId,
    node_addr: SocketAddr,
    config: ClusterConfig,
    registry: SharedNodeRegistry,
    gossip: Arc<SwimProtocol>,
    health_checker: Arc<HealthChecker>,
    pool: Arc<ConnectionPoolImpl>,
    event_broadcaster: ClusterEventBroadcaster,
    tags: Arc<RwLock<HashMap<String, String>>>,
    joined: Arc<tokio::sync::RwLock<bool>>,
    swim_acks_blocked: Arc<AtomicBool>,
}

impl ClusterMembership {
    pub async fn new(
        node_addr: SocketAddr,
        config: ClusterConfig,
        quic_client: Arc<QuicClient>,
    ) -> Result<Self, ClusterError> {
        let node_id = config
            .node_id
            .clone()
            .unwrap_or_else(|| NodeId::new(&format!("node-{}", node_addr)));

        let registry = SharedNodeRegistry::new();
        let event_broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let pool = Arc::new(ConnectionPoolImpl::new(config.pool.clone(), quic_client));
        let queue = GossipQueue::new(10);

        let gossip = Arc::new(SwimProtocol::new(
            node_id.clone(),
            node_addr,
            config.gossip.clone(),
            registry.clone(),
            queue,
            pool.clone(),
            event_broadcaster.clone(),
        ));

        let health_checker = Arc::new(HealthChecker::new(
            config.health.clone(),
            registry.clone(),
            event_broadcaster.clone(),
        ));

        let self_status = NodeStatus {
            node_id: node_id.clone(),
            addr: node_addr,
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };
        registry.insert(self_status);

        Ok(Self {
            node_id,
            node_addr,
            config,
            registry,
            gossip,
            health_checker,
            pool,
            event_broadcaster,
            tags: Arc::new(RwLock::new(HashMap::new())),
            joined: Arc::new(tokio::sync::RwLock::new(false)),
            swim_acks_blocked: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn join(&self, seeds: Vec<SocketAddr>) -> Result<(), ClusterError> {
        {
            let mut joined = self.joined.write().await;
            if *joined {
                return Err(ClusterError::AlreadyJoined);
            }
            *joined = true;
        }

        if !seeds.is_empty() {
            timeout(self.config.bootstrap_timeout, self.bootstrap(seeds))
                .await
                .map_err(|_| ClusterError::BootstrapTimeout(self.config.bootstrap_timeout))??;
        }

        let gossip = self.gossip.clone();
        tokio::spawn(async move {
            gossip.start().await;
        });

        let health_checker = self.health_checker.clone();
        tokio::spawn(async move {
            health_checker.start().await;
        });

        let pool = self.pool.clone();
        tokio::spawn(async move {
            pool.run_idle_cleanup().await;
        });

        Ok(())
    }

    async fn bootstrap(&self, seeds: Vec<SocketAddr>) -> Result<(), ClusterError> {
        for seed_addr in seeds {
            if seed_addr == self.node_addr {
                continue;
            }

            match self.pool.get_or_create(seed_addr).await {
                Ok(_conn) => {
                    self.pool.release(&seed_addr);
                    
                    let seed_node = NodeStatus {
                        node_id: NodeId::new(&format!("seed-{}", seed_addr)),
                        addr: seed_addr,
                        incarnation: Incarnation::initial(),
                        state: NodeState::Alive,
                        last_seen: Instant::now(),
                        tags: HashMap::new(),
                    };
                    self.registry.insert(seed_node);
                    
                    return Ok(());
                }
                Err(_) => continue,
            }
        }

        Err(ClusterError::NoSeedsReachable)
    }

    pub async fn register_self<I, K, V>(&self, tags: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let tags_map: HashMap<String, String> = tags
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        *self.tags.write().await = tags_map.clone();

        if let Some(mut self_status) = self.registry.get(&self.node_id) {
            self_status.tags = tags_map.clone();
            self_status.incarnation.increment();
            self.registry.insert(self_status.clone());

            let update = NodeUpdate {
                node_id: self.node_id.clone(),
                addr: self.node_addr,
                state: NodeState::Alive,
                incarnation: self_status.incarnation,
                tags: self_status.tags.clone(),
            };

            self.gossip.broadcast(update, Priority::High).await;

            self.event_broadcaster.send(ClusterEvent::NodeTagsUpdated {
                node_id: self.node_id.clone(),
                tags: self_status.tags,
            });
        }
    }

    pub async fn update_tags<I, K, V>(&self, tags: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let updates: HashMap<String, String> = tags
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        {
            let mut current_tags = self.tags.write().await;
            for (key, value) in updates.iter() {
                current_tags.insert(key.clone(), value.clone());
            }
        }

        if let Some(mut self_status) = self.registry.get(&self.node_id) {
            for (key, value) in updates.iter() {
                self_status.tags.insert(key.clone(), value.clone());
            }
            self_status.incarnation.increment();
            self.registry.insert(self_status.clone());

            let update = NodeUpdate {
                node_id: self.node_id.clone(),
                addr: self.node_addr,
                state: NodeState::Alive,
                incarnation: self_status.incarnation,
                tags: self_status.tags.clone(),
            };

            self.gossip.broadcast(update, Priority::High).await;

            self.event_broadcaster.send(ClusterEvent::NodeTagsUpdated {
                node_id: self.node_id.clone(),
                tags: self_status.tags,
            });
        }
    }

    pub async fn update_tag(&self, key: String, value: String) {
        self.tags.write().await.insert(key.clone(), value.clone());

        if let Some(mut self_status) = self.registry.get(&self.node_id) {
            self_status.tags.insert(key.clone(), value.clone());
            self_status.incarnation.increment();
            self.registry.insert(self_status.clone());

            let update = NodeUpdate {
                node_id: self.node_id.clone(),
                addr: self.node_addr,
                state: NodeState::Alive,
                incarnation: self_status.incarnation,
                tags: self_status.tags.clone(),
            };

            self.gossip.broadcast(update, Priority::High).await;

            self.event_broadcaster.send(ClusterEvent::NodeTagsUpdated {
                node_id: self.node_id.clone(),
                tags: self_status.tags,
            });
        }
    }

    pub async fn remove_tag(&self, key: &str) {
        self.tags.write().await.remove(key);

        if let Some(mut self_status) = self.registry.get(&self.node_id) {
            self_status.tags.remove(key);
            self_status.incarnation.increment();
            self.registry.insert(self_status.clone());

            let update = NodeUpdate {
                node_id: self.node_id.clone(),
                addr: self.node_addr,
                state: NodeState::Alive,
                incarnation: self_status.incarnation,
                tags: self_status.tags.clone(),
            };

            self.gossip.broadcast(update, Priority::High).await;

            self.event_broadcaster.send(ClusterEvent::NodeTagsUpdated {
                node_id: self.node_id.clone(),
                tags: self_status.tags,
            });
        }
    }

    pub async fn nodes_with_tag(&self, key: &str, value: &str) -> Vec<NodeStatus> {
        self.registry
            .all_nodes()
            .into_iter()
            .filter(|node| {
                node.state == NodeState::Alive
                    && node.tags.get(key).map(|v| v.as_str()) == Some(value)
            })
            .collect()
    }

    pub async fn nodes_with_all_tags(&self, tags: &HashMap<String, String>) -> Vec<NodeStatus> {
        self.registry
            .all_nodes()
            .into_iter()
            .filter(|node| {
                if node.state != NodeState::Alive {
                    return false;
                }
                tags.iter()
                    .all(|(k, v)| node.tags.get(k).map(|nv| nv.as_str()) == Some(v.as_str()))
            })
            .collect()
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    pub fn node_addr(&self) -> SocketAddr {
        self.node_addr
    }

    pub fn registry(&self) -> &SharedNodeRegistry {
        &self.registry
    }

    pub fn subscribe(&self) -> ClusterEventReceiver {
        self.event_broadcaster.subscribe()
    }

    pub async fn leave(&self) -> Result<(), ClusterError> {
        {
            let mut joined = self.joined.write().await;
            if !*joined {
                return Err(ClusterError::NotJoined);
            }
            *joined = false;
        }

        if let Some(mut self_status) = self.registry.get(&self.node_id) {
            self_status.state = NodeState::Left;
            self_status.incarnation.increment();
            self.registry.insert(self_status.clone());

            let update = NodeUpdate {
                node_id: self.node_id.clone(),
                addr: self.node_addr,
                state: NodeState::Left,
                incarnation: self_status.incarnation,
                tags: HashMap::new(),
            };

            self.gossip.broadcast(update, Priority::Critical).await;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        self.health_checker.stop().await;
        self.gossip.stop().await;

        self.event_broadcaster
            .send(ClusterEvent::NodeLeft(self.node_id.clone()));

        Ok(())
    }

    pub fn stats(&self) -> ClusterStats {
        let pool_stats = self.pool.stats();
        let all_nodes = self.registry.all_nodes();
        let alive_count = all_nodes
            .iter()
            .filter(|n| n.state == NodeState::Alive)
            .count();
        let suspect_count = all_nodes
            .iter()
            .filter(|n| n.state == NodeState::Suspect)
            .count();
        let failed_count = all_nodes
            .iter()
            .filter(|n| n.state == NodeState::Failed)
            .count();

        ClusterStats {
            total_nodes: all_nodes.len(),
            alive_nodes: alive_count,
            suspect_nodes: suspect_count,
            failed_nodes: failed_count,
            total_connections: pool_stats.total_connections,
            idle_connections: pool_stats.idle,
        }
    }

    pub fn stop_heartbeats(&self) {
        self.swim_acks_blocked.store(true, Ordering::SeqCst);
    }

    pub fn resume_heartbeats(&self) {
        self.swim_acks_blocked.store(false, Ordering::SeqCst);
    }

    pub fn should_block_swim_acks(&self) -> bool {
        self.swim_acks_blocked.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone)]
pub struct ClusterStats {
    pub total_nodes: usize,
    pub alive_nodes: usize,
    pub suspect_nodes: usize,
    pub failed_nodes: usize,
    pub total_connections: usize,
    pub idle_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    async fn test_cluster_membership_creation() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        assert_eq!(cluster.node_addr(), addr);
        assert_eq!(cluster.registry().len(), 1);
    }

    #[tokio::test]
    async fn test_update_tag() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8001".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        cluster.update_tag("role".to_string(), "worker".to_string()).await;

        let tags = cluster.tags.read().await;
        assert_eq!(tags.get("role"), Some(&"worker".to_string()));
    }

    #[tokio::test]
    async fn test_remove_tag() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8002".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        cluster.update_tag("role".to_string(), "worker".to_string()).await;
        cluster.remove_tag("role").await;

        let tags = cluster.tags.read().await;
        assert_eq!(tags.get("role"), None);
    }

    #[tokio::test]
    async fn test_nodes_with_tag() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8003".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        cluster.update_tag("role".to_string(), "worker".to_string()).await;

        let mut other_node = NodeStatus {
            node_id: NodeId::new("other-node"),
            addr: "127.0.0.1:9000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };
        other_node.tags.insert("role".to_string(), "worker".to_string());
        cluster.registry().insert(other_node);

        let workers = cluster.nodes_with_tag("role", "worker").await;
        assert_eq!(workers.len(), 2);
    }

    #[tokio::test]
    async fn test_stats() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8004".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        let stats = cluster.stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.alive_nodes, 1);
        assert_eq!(stats.suspect_nodes, 0);
        assert_eq!(stats.failed_nodes, 0);
    }

    #[tokio::test]
    async fn test_join_no_seeds() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8005".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        let result = cluster.join(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_double_join_error() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8006".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        cluster.join(vec![]).await.unwrap();
        let result = cluster.join(vec![]).await;

        assert!(matches!(result, Err(ClusterError::AlreadyJoined)));
    }

    #[tokio::test]
    async fn test_leave_before_join_error() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8007".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        let result = cluster.leave().await;
        assert!(matches!(result, Err(ClusterError::NotJoined)));
    }

    #[tokio::test]
    async fn test_nodes_with_all_tags() {
        let config = ClusterConfig::default();
        let addr: SocketAddr = "127.0.0.1:8008".parse().unwrap();
        let client = create_test_client().await;

        let cluster = ClusterMembership::new(addr, config, client).await.unwrap();

        let mut tags_map = HashMap::new();
        tags_map.insert("role".to_string(), "worker".to_string());
        tags_map.insert("zone".to_string(), "us-east".to_string());

        cluster.update_tag("role".to_string(), "worker".to_string()).await;
        cluster.update_tag("zone".to_string(), "us-east".to_string()).await;

        let mut other_node = NodeStatus {
            node_id: NodeId::new("other-node"),
            addr: "127.0.0.1:9001".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };
        other_node.tags.insert("role".to_string(), "worker".to_string());
        cluster.registry().insert(other_node);

        let nodes = cluster.nodes_with_all_tags(&tags_map).await;
        assert_eq!(nodes.len(), 1);
    }
}
