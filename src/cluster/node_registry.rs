use crate::cluster::incarnation::{resolve_conflict, NodeStatus};
use crate::cluster::gossip::{NodeId, NodeState};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait NodeRegistry: Send + Sync {
    fn insert(&self, status: NodeStatus);
    fn get(&self, node_id: &NodeId) -> Option<NodeStatus>;
    fn remove(&self, node_id: &NodeId) -> Option<NodeStatus>;
    fn all_nodes(&self) -> Vec<NodeStatus>;
    fn alive_nodes(&self) -> Vec<NodeStatus>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

#[derive(Clone)]
pub struct SharedNodeRegistry {
    inner: Arc<RwLock<HashMap<NodeId, NodeStatus>>>,
}

impl SharedNodeRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::with_capacity(capacity))),
        }
    }
}

impl Default for SharedNodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRegistry for SharedNodeRegistry {
    fn insert(&self, status: NodeStatus) {
        let mut registry = self.inner.write().unwrap();
        
        if let Some(existing) = registry.get(&status.node_id) {
            let winner = resolve_conflict(existing, &status);
            if std::ptr::eq(winner, &status) {
                registry.insert(status.node_id.clone(), status);
            }
        } else {
            registry.insert(status.node_id.clone(), status);
        }
    }

    fn get(&self, node_id: &NodeId) -> Option<NodeStatus> {
        let registry = self.inner.read().unwrap();
        registry.get(node_id).cloned()
    }

    fn remove(&self, node_id: &NodeId) -> Option<NodeStatus> {
        let mut registry = self.inner.write().unwrap();
        registry.remove(node_id)
    }

    fn all_nodes(&self) -> Vec<NodeStatus> {
        let registry = self.inner.read().unwrap();
        registry.values().cloned().collect()
    }

    fn alive_nodes(&self) -> Vec<NodeStatus> {
        let registry = self.inner.read().unwrap();
        registry
            .values()
            .filter(|status| status.state == NodeState::Alive)
            .cloned()
            .collect()
    }

    fn len(&self) -> usize {
        let registry = self.inner.read().unwrap();
        registry.len()
    }

    fn is_empty(&self) -> bool {
        let registry = self.inner.read().unwrap();
        registry.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::incarnation::Incarnation;
    use std::collections::HashMap;
    
    use std::time::Instant;

    fn create_node_status(id: &str, _incarnation: u64, state: NodeState) -> NodeStatus {
        NodeStatus {
            node_id: NodeId::new(id),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        }
    }
    
    fn create_node_status_with_incarnation(id: &str, incarnation_value: u64, state: NodeState) -> NodeStatus {
        NodeStatus {
            node_id: NodeId::new(id),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::from_value(incarnation_value),
            state,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        }
    }

    #[test]
    fn test_insert_and_get() {
        let registry = SharedNodeRegistry::new();
        let status = create_node_status("node-1", 100, NodeState::Alive);
        
        registry.insert(status.clone());
        
        let retrieved = registry.get(&NodeId::new("node-1"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().node_id.as_str(), "node-1");
    }

    #[test]
    fn test_remove() {
        let registry = SharedNodeRegistry::new();
        let status = create_node_status("node-1", 100, NodeState::Alive);
        
        registry.insert(status);
        assert_eq!(registry.len(), 1);
        
        let removed = registry.remove(&NodeId::new("node-1"));
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_all_nodes() {
        let registry = SharedNodeRegistry::new();
        
        registry.insert(create_node_status("node-1", 100, NodeState::Alive));
        registry.insert(create_node_status("node-2", 200, NodeState::Suspect));
        registry.insert(create_node_status("node-3", 300, NodeState::Failed));
        
        let all = registry.all_nodes();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_alive_nodes() {
        let registry = SharedNodeRegistry::new();
        
        registry.insert(create_node_status("node-1", 100, NodeState::Alive));
        registry.insert(create_node_status("node-2", 200, NodeState::Suspect));
        registry.insert(create_node_status("node-3", 300, NodeState::Alive));
        registry.insert(create_node_status("node-4", 400, NodeState::Failed));
        
        let alive = registry.alive_nodes();
        assert_eq!(alive.len(), 2);
        
        let alive_ids: Vec<_> = alive.iter().map(|s| s.node_id.as_str()).collect();
        assert!(alive_ids.contains(&"node-1"));
        assert!(alive_ids.contains(&"node-3"));
    }

    #[test]
    fn test_incarnation_conflict_resolution() {
        let registry = SharedNodeRegistry::new();
        
        let mut status_old = create_node_status("node-1", 100, NodeState::Alive);
        status_old.incarnation = Incarnation::initial();
        
        registry.insert(status_old.clone());
        
        let mut status_new = create_node_status("node-1", 200, NodeState::Suspect);
        status_new.incarnation = status_old.incarnation;
        status_new.incarnation.increment();
        
        registry.insert(status_new.clone());
        
        let retrieved = registry.get(&NodeId::new("node-1")).unwrap();
        assert_eq!(retrieved.state, NodeState::Suspect);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(SharedNodeRegistry::with_capacity(100));
        let mut handles = vec![];

        for i in 0..10 {
            let reg = registry.clone();
            let handle = thread::spawn(move || {
                for j in 0..10 {
                    let id = format!("node-{}-{}", i, j);
                    reg.insert(create_node_status(&id, (i * 10 + j) as u64, NodeState::Alive));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 100);
    }

    #[test]
    fn test_clone_shares_data() {
        let registry = SharedNodeRegistry::new();
        let cloned = registry.clone();
        
        registry.insert(create_node_status("node-1", 100, NodeState::Alive));
        
        assert_eq!(cloned.len(), 1);
        assert!(cloned.get(&NodeId::new("node-1")).is_some());
    }

    #[test]
    fn test_is_empty() {
        let registry = SharedNodeRegistry::new();
        assert!(registry.is_empty());
        
        registry.insert(create_node_status("node-1", 100, NodeState::Alive));
        assert!(!registry.is_empty());
        
        registry.remove(&NodeId::new("node-1"));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_state_transitions() {
        let registry = SharedNodeRegistry::new();
        let node_id = NodeId::new("node-1");
        
        let mut status = create_node_status("node-1", 100, NodeState::Alive);
        registry.insert(status.clone());
        
        status.state = NodeState::Suspect;
        status.incarnation.increment();
        registry.insert(status.clone());
        assert_eq!(registry.get(&node_id).unwrap().state, NodeState::Suspect);
        
        status.state = NodeState::Failed;
        status.incarnation.increment();
        registry.insert(status.clone());
        assert_eq!(registry.get(&node_id).unwrap().state, NodeState::Failed);
    }

    #[test]
    fn test_higher_incarnation_wins() {
        let registry = SharedNodeRegistry::new();
        let node_id = NodeId::new("node-1");
        
        let status_low = create_node_status_with_incarnation("node-1", 0, NodeState::Alive);
        let status_high = create_node_status_with_incarnation("node-1", 2, NodeState::Suspect);
        
        registry.insert(status_low.clone());
        registry.insert(status_high.clone());
        
        let result = registry.get(&node_id).unwrap();
        assert_eq!(result.incarnation.value(), 2);
        assert_eq!(result.state, NodeState::Suspect);
    }

    #[test]
    fn test_lower_incarnation_rejected() {
        let registry = SharedNodeRegistry::new();
        let node_id = NodeId::new("node-1");
        
        let status_high = create_node_status_with_incarnation("node-1", 2, NodeState::Alive);
        let status_low = create_node_status_with_incarnation("node-1", 0, NodeState::Suspect);
        
        registry.insert(status_high.clone());
        registry.insert(status_low.clone());
        
        let result = registry.get(&node_id).unwrap();
        assert_eq!(result.incarnation.value(), 2);
        assert_eq!(result.state, NodeState::Alive);
    }

    #[test]
    fn test_same_incarnation_alive_wins() {
        let registry = SharedNodeRegistry::new();
        let node_id = NodeId::new("node-1");
        
        let status_suspect = create_node_status_with_incarnation("node-1", 0, NodeState::Suspect);
        let status_alive = create_node_status_with_incarnation("node-1", 0, NodeState::Alive);
        
        registry.insert(status_suspect.clone());
        registry.insert(status_alive.clone());
        
        let result = registry.get(&node_id).unwrap();
        assert_eq!(result.state, NodeState::Alive);
    }

    #[test]
    fn test_conflict_resolution_suspect_vs_failed() {
        let registry = SharedNodeRegistry::new();
        let node_id = NodeId::new("node-1");
        
        let status_suspect = create_node_status_with_incarnation("node-1", 0, NodeState::Suspect);
        let status_failed = create_node_status_with_incarnation("node-1", 0, NodeState::Failed);
        
        registry.insert(status_suspect.clone());
        registry.insert(status_failed.clone());
        
        let result = registry.get(&node_id).unwrap();
        assert_eq!(result.state, NodeState::Failed);
    }
}
