use super::message::{NodeId, NodeUpdate, Priority, MAX_UPDATES_PER_MESSAGE};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};

pub struct GossipQueue {
    updates: Arc<RwLock<BTreeMap<(Priority, NodeId), NodeUpdate>>>,
    seen_count: Arc<RwLock<HashMap<NodeId, usize>>>,
    cluster_size: Arc<AtomicUsize>,
}

impl GossipQueue {
    pub fn new(cluster_size: usize) -> Self {
        Self {
            updates: Arc::new(RwLock::new(BTreeMap::new())),
            seen_count: Arc::new(RwLock::new(HashMap::new())),
            cluster_size: Arc::new(AtomicUsize::new(cluster_size.max(1))),
        }
    }

    pub fn enqueue(&self, update: NodeUpdate, priority: Priority) {
        let node_id = update.node_id.clone();
        self.updates
            .write()
            .unwrap()
            .insert((priority, node_id), update);
    }

    pub fn select_updates(&self) -> Vec<NodeUpdate> {
        let cluster_size = self.cluster_size.load(AtomicOrdering::Acquire);
        let max_rounds = (cluster_size as f64).log2().ceil() as usize * 3;
        let seen_counts = self.seen_count.read().unwrap().clone();

        let mut selected = Vec::new();
        let mut to_remove = Vec::new();

        {
            let updates = self.updates.read().unwrap();
            for ((priority, node_id), update) in updates.iter().rev() {
                if selected.len() >= MAX_UPDATES_PER_MESSAGE {
                    break;
                }

                let sent_count = seen_counts.get(node_id).copied().unwrap_or(0);
                if sent_count < max_rounds {
                    selected.push(update.clone());
                }

                to_remove.push((*priority, node_id.clone()));
            }
        }

        let mut updates = self.updates.write().unwrap();
        for key in to_remove {
            updates.remove(&key);
        }

        selected
    }

    pub fn mark_sent(&self, node_id: &NodeId) {
        let mut seen = self.seen_count.write().unwrap();
        *seen.entry(node_id.clone()).or_insert(0) += 1;
    }

    pub fn should_stop_gossiping(&self, node_id: &NodeId) -> bool {
        let cluster_size = self.cluster_size.load(AtomicOrdering::Acquire);
        let max_rounds = (cluster_size as f64).log2().ceil() as usize * 3;
        let seen = self.seen_count.read().unwrap();
        seen.get(node_id).copied().unwrap_or(0) >= max_rounds
    }

    pub fn update_cluster_size(&self, size: usize) {
        self.cluster_size
            .store(size.max(1), AtomicOrdering::Release);
    }

    pub fn len(&self) -> usize {
        self.updates.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.updates.read().unwrap().is_empty()
    }

    pub fn clear_seen_counts(&self) {
        self.seen_count.write().unwrap().clear();
    }
}

impl Clone for GossipQueue {
    fn clone(&self) -> Self {
        Self {
            updates: self.updates.clone(),
            seen_count: self.seen_count.clone(),
            cluster_size: self.cluster_size.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::gossip::message::NodeState;
    use crate::cluster::incarnation::Incarnation;
    use std::collections::HashMap as StdHashMap;

    fn create_test_update(id: &str) -> NodeUpdate {
        NodeUpdate {
            node_id: NodeId::new(id),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            tags: StdHashMap::new(),
        }
    }

    #[test]
    fn test_priority_ordering() {
        let queue = GossipQueue::new(10);

        queue.enqueue(create_test_update("low"), Priority::Low);
        queue.enqueue(create_test_update("high"), Priority::High);
        queue.enqueue(create_test_update("critical"), Priority::Critical);
        queue.enqueue(create_test_update("medium"), Priority::Medium);

        let selected = queue.select_updates();

        assert_eq!(selected.len(), 4);
        assert_eq!(selected[0].node_id.as_str(), "critical");
        assert_eq!(selected[1].node_id.as_str(), "high");
        assert_eq!(selected[2].node_id.as_str(), "medium");
        assert_eq!(selected[3].node_id.as_str(), "low");
    }

    #[test]
    fn test_max_updates_limit() {
        let queue = GossipQueue::new(10);

        for i in 0..30 {
            queue.enqueue(create_test_update(&format!("node-{}", i)), Priority::Medium);
        }

        let selected = queue.select_updates();
        assert_eq!(selected.len(), MAX_UPDATES_PER_MESSAGE);
    }

    #[test]
    fn test_gossip_redundancy_elimination() {
        let queue = GossipQueue::new(10);
        let node_id = NodeId::new("test-node");

        queue.enqueue(create_test_update("test-node"), Priority::High);

        let max_rounds = (10_f64.log2().ceil() as usize) * 3;
        for _ in 0..max_rounds {
            queue.mark_sent(&node_id);
        }

        assert!(queue.should_stop_gossiping(&node_id));
    }

    #[test]
    fn test_gossip_rounds_calculation() {
        let queue = GossipQueue::new(100);

        let max_rounds = (100_f64.log2().ceil() as usize) * 3;
        assert_eq!(max_rounds, 21);

        let node_id = NodeId::new("test");

        for _i in 0..max_rounds {
            assert!(!queue.should_stop_gossiping(&node_id));
            queue.mark_sent(&node_id);
        }

        assert!(queue.should_stop_gossiping(&node_id));
    }

    #[test]
    fn test_cluster_size_update() {
        let queue = GossipQueue::new(10);

        assert_eq!(queue.cluster_size.load(AtomicOrdering::Acquire), 10);

        queue.update_cluster_size(50);
        assert_eq!(queue.cluster_size.load(AtomicOrdering::Acquire), 50);
    }

    #[test]
    fn test_empty_queue() {
        let queue = GossipQueue::new(10);

        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        let selected = queue.select_updates();
        assert!(selected.is_empty());
    }

    #[test]
    fn test_clear_seen_counts() {
        let queue = GossipQueue::new(10);
        let node_id = NodeId::new("test");

        for _ in 0..5 {
            queue.mark_sent(&node_id);
        }

        assert_eq!(
            queue
                .seen_count
                .read()
                .unwrap()
                .get(&node_id)
                .copied()
                .unwrap(),
            5
        );

        queue.clear_seen_counts();
        assert_eq!(
            queue
                .seen_count
                .read()
                .unwrap()
                .get(&node_id)
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn test_priority_replacement() {
        let queue = GossipQueue::new(10);

        queue.enqueue(create_test_update("node-1"), Priority::Low);
        queue.enqueue(create_test_update("node-1"), Priority::Critical);

        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let queue = Arc::new(GossipQueue::new(100));
        let mut handles = vec![];

        for i in 0..10 {
            let q = queue.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let update = create_test_update(&format!("node-{}-{}", i, j));
                    q.enqueue(update, Priority::Medium);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(queue.len(), 1000);
    }

    #[test]
    fn test_concurrent_select_and_enqueue() {
        use std::thread;

        let queue = Arc::new(GossipQueue::new(100));

        for i in 0..50 {
            let update = create_test_update(&format!("initial-{}", i));
            queue.enqueue(update, Priority::High);
        }

        let q1 = queue.clone();
        let enqueue_handle = thread::spawn(move || {
            for i in 0..100 {
                let update = create_test_update(&format!("concurrent-{}", i));
                q1.enqueue(update, Priority::Medium);
            }
        });

        let q2 = queue.clone();
        let select_handle = thread::spawn(move || {
            let mut total_selected = 0;
            for _ in 0..10 {
                let selected = q2.select_updates();
                total_selected += selected.len();
                thread::sleep(std::time::Duration::from_millis(1));
            }
            total_selected
        });

        enqueue_handle.join().unwrap();
        let total_selected = select_handle.join().unwrap();

        assert!(total_selected > 0);
    }
}
