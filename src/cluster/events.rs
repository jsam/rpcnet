use crate::cluster::gossip::NodeId;
use crate::cluster::partition_detector::PartitionStatus;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;

pub const EVENT_CHANNEL_CAPACITY: usize = 1000;

#[derive(Debug, Clone)]
pub enum ClusterEvent {
    NodeJoined(ClusterNode),
    NodeLeft(NodeId),
    NodeFailed(NodeId),
    NodeRecovered(NodeId),
    NodeTagsUpdated {
        node_id: NodeId,
        tags: HashMap<String, String>,
    },
    PartitionDetected {
        status: PartitionStatus,
    },
    EventsDropped {
        count: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ClusterNode {
    pub id: NodeId,
    pub addr: std::net::SocketAddr,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum RecvError {
    #[error("Consumer lagged, {0} events dropped")]
    Lagged(u64),

    #[error("Channel closed")]
    Closed,
}

pub struct ClusterEventBroadcaster {
    tx: broadcast::Sender<ClusterEvent>,
    drops: Arc<AtomicU64>,
}

impl ClusterEventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            drops: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(EVENT_CHANNEL_CAPACITY)
    }

    pub fn send(&self, event: ClusterEvent) {
        if self.tx.receiver_count() > 0 {
            if let Err(_) = self.tx.send(event) {
                let current_drops = self.drops.fetch_add(1, Ordering::SeqCst) + 1;
                
                if current_drops % 100 == 0 {
                    let _ = self.tx.send(ClusterEvent::EventsDropped {
                        count: current_drops,
                    });
                }
            }
        }
    }

    pub fn subscribe(&self) -> ClusterEventReceiver {
        ClusterEventReceiver {
            inner: self.tx.subscribe(),
            drops: self.drops.clone(),
        }
    }

    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    pub fn total_drops(&self) -> u64 {
        self.drops.load(Ordering::SeqCst)
    }
}

impl Clone for ClusterEventBroadcaster {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            drops: self.drops.clone(),
        }
    }
}

pub struct ClusterEventReceiver {
    inner: broadcast::Receiver<ClusterEvent>,
    drops: Arc<AtomicU64>,
}

impl ClusterEventReceiver {
    pub async fn recv(&mut self) -> Result<ClusterEvent, RecvError> {
        match self.inner.recv().await {
            Ok(event) => Ok(event),
            Err(broadcast::error::RecvError::Lagged(n)) => {
                self.drops.fetch_add(n, Ordering::SeqCst);
                Err(RecvError::Lagged(n))
            }
            Err(broadcast::error::RecvError::Closed) => Err(RecvError::Closed),
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.drops.load(Ordering::SeqCst)
    }

    pub fn try_recv(&mut self) -> Result<ClusterEvent, broadcast::error::TryRecvError> {
        self.inner.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_send_receive() {
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let mut receiver = broadcaster.subscribe();

        let node_id = NodeId::new("test-node");
        broadcaster.send(ClusterEvent::NodeFailed(node_id.clone()));

        let event = receiver.recv().await.unwrap();
        match event {
            ClusterEvent::NodeFailed(id) => assert_eq!(id.as_str(), "test-node"),
            _ => panic!("Expected NodeFailed event"),
        }
    }

    #[tokio::test]
    async fn test_multiple_receivers() {
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let mut receiver1 = broadcaster.subscribe();
        let mut receiver2 = broadcaster.subscribe();

        assert_eq!(broadcaster.receiver_count(), 2);

        let node_id = NodeId::new("test-node");
        broadcaster.send(ClusterEvent::NodeFailed(node_id.clone()));

        let event1 = receiver1.recv().await.unwrap();
        let event2 = receiver2.recv().await.unwrap();

        assert!(matches!(event1, ClusterEvent::NodeFailed(_)));
        assert!(matches!(event2, ClusterEvent::NodeFailed(_)));
    }

    #[tokio::test]
    async fn test_lagged_receiver() {
        let broadcaster = ClusterEventBroadcaster::new(10);
        let mut receiver = broadcaster.subscribe();

        for i in 0..20 {
            broadcaster.send(ClusterEvent::NodeFailed(NodeId::new(format!("node-{}", i))));
        }

        let mut received = 0;
        let mut lagged = false;

        loop {
            match receiver.recv().await {
                Ok(_) => received += 1,
                Err(RecvError::Lagged(n)) => {
                    lagged = true;
                    assert!(n > 0);
                }
                Err(RecvError::Closed) => break,
            }

            if received >= 10 {
                break;
            }
        }

        assert!(lagged, "Receiver should have lagged");
        assert!(receiver.dropped_count() > 0);
    }

    #[tokio::test]
    async fn test_dropped_count_tracking() {
        let broadcaster = ClusterEventBroadcaster::new(5);
        let mut receiver = broadcaster.subscribe();

        for i in 0..10 {
            broadcaster.send(ClusterEvent::NodeFailed(NodeId::new(format!("node-{}", i))));
        }

        let _ = receiver.recv().await;

        assert!(receiver.dropped_count() > 0);
        assert!(broadcaster.total_drops() > 0);
    }

    #[tokio::test]
    async fn test_no_receivers() {
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();

        assert_eq!(broadcaster.receiver_count(), 0);

        broadcaster.send(ClusterEvent::NodeFailed(NodeId::new("test")));
    }

    #[tokio::test]
    async fn test_receiver_closed() {
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let mut receiver = broadcaster.subscribe();

        drop(broadcaster);

        let result = receiver.recv().await;
        assert!(matches!(result, Err(RecvError::Closed)));
    }

    #[tokio::test]
    async fn test_try_recv() {
        let broadcaster = ClusterEventBroadcaster::with_default_capacity();
        let mut receiver = broadcaster.subscribe();

        let result = receiver.try_recv();
        assert!(matches!(
            result,
            Err(broadcast::error::TryRecvError::Empty)
        ));

        broadcaster.send(ClusterEvent::NodeFailed(NodeId::new("test")));

        let result = receiver.try_recv();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cluster_node() {
        let mut tags = HashMap::new();
        tags.insert("role".to_string(), "worker".to_string());

        let node = ClusterNode {
            id: NodeId::new("node-1"),
            addr: "127.0.0.1:8000".parse().unwrap(),
            tags: tags.clone(),
        };

        assert_eq!(node.id.as_str(), "node-1");
        assert_eq!(node.tags.get("role").unwrap(), "worker");
    }

    #[test]
    fn test_broadcaster_clone() {
        let broadcaster1 = ClusterEventBroadcaster::with_default_capacity();
        let broadcaster2 = broadcaster1.clone();

        assert_eq!(
            Arc::ptr_eq(&broadcaster1.drops, &broadcaster2.drops),
            true
        );
    }
}
