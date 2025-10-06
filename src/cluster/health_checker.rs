use crate::cluster::events::{ClusterEvent, ClusterEventBroadcaster};
use crate::cluster::failure_detection::phi_accrual::PhiAccrualDetector;
use crate::cluster::gossip::{NodeId, NodeState};
use crate::cluster::node_registry::{NodeRegistry, SharedNodeRegistry};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_interval: Duration,
    pub phi_threshold: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            phi_threshold: 8.0,
        }
    }
}

pub struct HealthChecker {
    config: HealthCheckConfig,
    registry: SharedNodeRegistry,
    event_broadcaster: ClusterEventBroadcaster,
    detectors: Arc<RwLock<HashMap<NodeId, PhiAccrualDetector>>>,
    running: Arc<AtomicBool>,
}

impl HealthChecker {
    pub fn new(
        config: HealthCheckConfig,
        registry: SharedNodeRegistry,
        event_broadcaster: ClusterEventBroadcaster,
    ) -> Self {
        Self {
            config,
            registry,
            event_broadcaster,
            detectors: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) {
        self.running.store(true, Ordering::SeqCst);

        while self.running.load(Ordering::SeqCst) {
            self.check_health().await;
            sleep(self.config.check_interval).await;
        }
    }

    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    async fn check_health(&self) {
        let nodes = self.registry.alive_nodes();

        for node in nodes {
            let mut detectors = self.detectors.write().await;
            let detector = detectors
                .entry(node.node_id.clone())
                .or_insert_with(|| PhiAccrualDetector::new(self.config.phi_threshold, 100, 5));

            let phi = detector.phi();

            if phi > self.config.phi_threshold {
                drop(detectors);

                let mut suspect_node = node.clone();
                suspect_node.state = NodeState::Suspect;
                suspect_node.incarnation.increment();

                self.registry.insert(suspect_node);

                self.event_broadcaster
                    .send(ClusterEvent::NodeFailed(node.node_id.clone()));
            }
        }
    }

    pub async fn heartbeat(&self, node_id: &NodeId) {
        let mut detectors = self.detectors.write().await;
        if let Some(detector) = detectors.get_mut(node_id) {
            detector.heartbeat();
        } else {
            let mut detector = PhiAccrualDetector::new(8.0, 100, 5);
            detector.heartbeat();
            detectors.insert(node_id.clone(), detector);
        }
    }

    pub async fn phi(&self, node_id: &NodeId) -> f64 {
        let detectors = self.detectors.read().await;
        detectors.get(node_id).map(|d| d.phi()).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::incarnation::{Incarnation, NodeStatus};

    use std::time::Instant;

    #[tokio::test]
    async fn test_health_checker_creation() {
        let config = HealthCheckConfig::default();
        let registry = SharedNodeRegistry::new();
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let checker = HealthChecker::new(config, registry, broadcaster);

        assert!(!checker.running.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_heartbeat_tracking() {
        let config = HealthCheckConfig::default();
        let registry = SharedNodeRegistry::new();
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let checker = HealthChecker::new(config, registry, broadcaster);

        let node_id = NodeId::new("test-node");
        checker.heartbeat(&node_id).await;

        let phi = checker.phi(&node_id).await;
        assert!(phi >= 0.0);
    }

    #[tokio::test]
    async fn test_phi_threshold_detection() {
        let config = HealthCheckConfig {
            check_interval: Duration::from_millis(10),
            phi_threshold: 8.0,
        };
        let registry = SharedNodeRegistry::new();
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let mut receiver = broadcaster.subscribe();

        let checker = HealthChecker::new(config, registry.clone(), broadcaster);

        let node_id = NodeId::new("failing-node");
        let status = NodeStatus {
            node_id: node_id.clone(),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        registry.insert(status);

        checker.heartbeat(&node_id).await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        checker.check_health().await;

        let event = tokio::time::timeout(Duration::from_millis(200), receiver.recv())
            .await
            .ok()
            .and_then(|r| r.ok());

        if let Some(ClusterEvent::NodeFailed(failed_id)) = event {
            assert_eq!(failed_id.as_str(), "failing-node");

            let updated = registry.get(&node_id).unwrap();
            assert_eq!(updated.state, NodeState::Suspect);
        }
    }

    #[tokio::test]
    async fn test_stop_health_checker() {
        let config = HealthCheckConfig::default();
        let registry = SharedNodeRegistry::new();
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let checker = Arc::new(HealthChecker::new(config, registry, broadcaster));

        let checker_clone = checker.clone();
        let handle = tokio::spawn(async move {
            checker_clone.start().await;
        });

        tokio::time::sleep(Duration::from_millis(10)).await;

        checker.stop().await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        assert!(!checker.running.load(Ordering::SeqCst));

        handle.abort();
    }

    #[tokio::test]
    async fn test_multiple_heartbeats_lower_phi() {
        let config = HealthCheckConfig::default();
        let registry = SharedNodeRegistry::new();
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        let checker = HealthChecker::new(config, registry, broadcaster);

        let node_id = NodeId::new("healthy-node");

        checker.heartbeat(&node_id).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        checker.heartbeat(&node_id).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        checker.heartbeat(&node_id).await;

        let phi = checker.phi(&node_id).await;

        assert!(phi < 8.0);
    }
}
