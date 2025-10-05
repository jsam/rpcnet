use crate::cluster::connection_pool::ConnectionPool;
use crate::cluster::events::{ClusterEvent, ClusterEventBroadcaster};
use crate::cluster::gossip::{
    config::GossipConfig, swim::SwimMessage, GossipQueue, NodeId, NodeState, NodeUpdate, Priority,
};
use crate::cluster::incarnation::NodeStatus;
use crate::cluster::node_registry::{NodeRegistry, SharedNodeRegistry};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, timeout};

#[async_trait]
pub trait GossipProtocol: Send + Sync {
    async fn start(&self);
    async fn stop(&self);
    async fn broadcast(&self, update: NodeUpdate, priority: Priority);
    fn select_random_nodes(&self, count: usize) -> Vec<NodeStatus>;
}

pub struct SwimProtocol {
    node_id: NodeId,
    node_addr: SocketAddr,
    config: GossipConfig,
    registry: SharedNodeRegistry,
    queue: GossipQueue,
    connection_pool: Arc<dyn ConnectionPool>,
    event_broadcaster: ClusterEventBroadcaster,
    running: Arc<AtomicBool>,
    seq_counter: Arc<AtomicU64>,
}

impl SwimProtocol {
    pub fn new(
        node_id: NodeId,
        node_addr: SocketAddr,
        config: GossipConfig,
        registry: SharedNodeRegistry,
        queue: GossipQueue,
        connection_pool: Arc<dyn ConnectionPool>,
        event_broadcaster: ClusterEventBroadcaster,
    ) -> Self {
        Self {
            node_id,
            node_addr,
            config,
            registry,
            queue,
            connection_pool,
            event_broadcaster,
            running: Arc::new(AtomicBool::new(false)),
            seq_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq_counter.fetch_add(1, Ordering::SeqCst)
    }

    async fn protocol_period(&self) {
        use tracing::debug;
        
        let nodes = self.registry.alive_nodes();
        
        debug!("🔄 [SWIM] Protocol period: {} alive nodes in registry", nodes.len());
        
        if nodes.is_empty() {
            debug!("⚠️  [SWIM] No alive nodes to ping");
            return;
        }

        let target = {
            let mut rng = rand::thread_rng();
            nodes.choose(&mut rng).cloned()
        };

        if let Some(target) = target {
            if target.node_id == self.node_id {
                debug!("⚠️  [SWIM] Skipping self-ping");
                return;
            }

            let seq = self.next_seq();
            let updates = self.queue.select_updates();
            
            debug!("📤 [SWIM] Sending Ping to {:?} at {} (seq={}, {} updates)", 
                  target.node_id, target.addr, seq, updates.len());
            
            let ping = SwimMessage::Ping {
                from: self.node_id.clone(),
                from_addr: self.node_addr,
                updates,
                seq,
            };

            let ack_received = self.send_ping(&target, ping.clone()).await;

            if ack_received {
                debug!("✅ [SWIM] Received ACK from {:?}", target.node_id);
            } else {
                debug!("⚠️  [SWIM] No ACK from {:?}, trying indirect ping", target.node_id);
                self.indirect_ping(&target, ping).await;
            }
        }
    }

    async fn send_ping(&self, target: &NodeStatus, ping: SwimMessage) -> bool {
        match timeout(
            self.config.ack_timeout,
            self.send_message(target.addr, &ping),
        )
        .await
        {
            Ok(Ok(Some(SwimMessage::Ack { .. }))) => true,
            _ => false,
        }
    }

    async fn indirect_ping(&self, target: &NodeStatus, original_ping: SwimMessage) {
        let intermediaries = self.select_intermediaries(target, self.config.indirect_ping_count);

        if intermediaries.is_empty() {
            self.mark_suspect(target).await;
            return;
        }

        let ping_req = SwimMessage::PingReq {
            from: self.node_id.clone(),
            target: target.addr,
            target_id: target.node_id.clone(),
            updates: original_ping.updates().to_vec(),
            seq: original_ping.seq(),
        };

        let mut tasks = Vec::new();
        for intermediary in intermediaries {
            let ping_req = ping_req.clone();
            let pool = self.connection_pool.clone();
            let timeout_dur = self.config.indirect_timeout;
            
            tasks.push(tokio::spawn(async move {
                timeout(timeout_dur, Self::send_message_static(pool, intermediary.addr, &ping_req))
                    .await
                    .ok()
                    .and_then(|r| r.ok())
                    .and_then(|msg| msg)
            }));
        }

        let results = futures::future::join_all(tasks).await;
        
        let ack_received = results.iter().any(|r| {
            matches!(
                r,
                Ok(Some(SwimMessage::Ack { .. }))
            )
        });

        if !ack_received {
            self.mark_suspect(target).await;
        }
    }

    async fn mark_suspect(&self, target: &NodeStatus) {
        let mut updated = target.clone();
        updated.state = NodeState::Suspect;
        updated.incarnation.increment();

        self.registry.insert(updated.clone());

        let update = NodeUpdate {
            node_id: target.node_id.clone(),
            addr: target.addr,
            incarnation: updated.incarnation,
            state: NodeState::Suspect,
            tags: target.tags.clone(),
        };

        self.queue.enqueue(update, Priority::High);
        
        self.event_broadcaster
            .send(ClusterEvent::NodeFailed(target.node_id.clone()));
    }

    fn select_intermediaries(&self, exclude: &NodeStatus, count: usize) -> Vec<NodeStatus> {
        let nodes: Vec<_> = self
            .registry
            .alive_nodes()
            .into_iter()
            .filter(|n| n.node_id != exclude.node_id && n.node_id != self.node_id)
            .collect();

        let mut rng = rand::thread_rng();
        nodes
            .choose_multiple(&mut rng, count)
            .cloned()
            .collect()
    }

    async fn send_message(
        &self,
        addr: SocketAddr,
        msg: &SwimMessage,
    ) -> Result<Option<SwimMessage>, Box<dyn std::error::Error + Send + Sync>> {
        Self::send_message_static(self.connection_pool.clone(), addr, msg).await
    }

    async fn send_message_static(
        pool: Arc<dyn ConnectionPool>,
        addr: SocketAddr,
        msg: &SwimMessage,
    ) -> Result<Option<SwimMessage>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = pool.get_or_create(addr).await?;
        
        let bytes = msg.serialize()?;
        
        let mut conn_guard = conn.connection.lock().await;
        let mut stream = conn_guard.open_bidirectional_stream().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        stream.send(bytes.into()).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        
        let response = if let Some(data) = stream.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)? {
            SwimMessage::deserialize(&data).map(Some).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        } else {
            None
        };
        drop(conn_guard);
        
        pool.release(&addr);
        Ok(response)
    }
}

#[async_trait]
impl GossipProtocol for SwimProtocol {
    async fn start(&self) {
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let period = self.config.protocol_period;
        
        while running.load(Ordering::SeqCst) {
            self.protocol_period().await;
            sleep(period).await;
        }
    }

    async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    async fn broadcast(&self, update: NodeUpdate, priority: Priority) {
        self.queue.enqueue(update, priority);
    }

    fn select_random_nodes(&self, count: usize) -> Vec<NodeStatus> {
        let nodes = self.registry.alive_nodes();
        let mut rng = rand::thread_rng();
        nodes.choose_multiple(&mut rng, count).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::connection_pool::{ConnectionPoolImpl, PoolConfig};
    use std::collections::HashMap;
    use std::path::Path;
    use s2n_quic::Client as QuicClient;

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
    async fn test_swim_protocol_creation() {
        let node_id = NodeId::new("test-node");
        let node_addr = "127.0.0.1:9000".parse().unwrap();
        let config = GossipConfig::default();
        let registry = SharedNodeRegistry::new();
        let queue = GossipQueue::new(10);
        let client = create_test_client().await;
        let pool = Arc::new(ConnectionPoolImpl::new(PoolConfig::new(), client));
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let protocol = SwimProtocol::new(
            node_id,
            node_addr,
            config,
            registry,
            queue,
            pool,
            broadcaster,
        );

        assert_eq!(protocol.node_id.as_str(), "test-node");
    }

    #[tokio::test]
    async fn test_select_random_nodes() {
        let node_id = NodeId::new("test-node");
        let node_addr = "127.0.0.1:9000".parse().unwrap();
        let config = GossipConfig::default();
        let registry = SharedNodeRegistry::new();
        let queue = GossipQueue::new(10);
        let client = create_test_client().await;
        let pool = Arc::new(ConnectionPoolImpl::new(PoolConfig::new(), client));
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let protocol = SwimProtocol::new(
            node_id,
            node_addr,
            config,
            registry.clone(),
            queue,
            pool,
            broadcaster,
        );

        for i in 0..5 {
            let status = NodeStatus {
                node_id: NodeId::new(format!("node-{}", i)),
                addr: format!("127.0.0.1:900{}", i).parse().unwrap(),
                incarnation: crate::cluster::incarnation::Incarnation::initial(),
                state: NodeState::Alive,
                last_seen: std::time::Instant::now(),
                tags: HashMap::new(),
            };
            registry.insert(status);
        }

        let selected = protocol.select_random_nodes(3);
        assert!(selected.len() <= 3);
    }
}
