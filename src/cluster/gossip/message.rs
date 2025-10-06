use crate::cluster::incarnation::Incarnation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use thiserror::Error;

pub const MAX_UPDATES_PER_MESSAGE: usize = 20;
pub const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Critical = 3,
    High = 2,
    Medium = 1,
    Low = 0,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeUpdate {
    pub node_id: NodeId,
    pub addr: SocketAddr,
    pub incarnation: Incarnation,
    pub state: NodeState,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Alive,
    Suspect,
    Failed,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMessage {
    pub updates: Vec<NodeUpdate>,
}

impl GossipMessage {
    pub fn new(updates: Vec<NodeUpdate>) -> Self {
        Self { updates }
    }

    pub fn check_size(&self) -> Result<(), GossipError> {
        if self.updates.len() > MAX_UPDATES_PER_MESSAGE {
            return Err(GossipError::TooManyUpdates {
                count: self.updates.len(),
                max: MAX_UPDATES_PER_MESSAGE,
            });
        }

        let serialized =
            bincode::serialize(self).map_err(|e| GossipError::SerializationError { source: e })?;

        if serialized.len() > MAX_MESSAGE_SIZE {
            return Err(GossipError::MessageTooLarge {
                size: serialized.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum GossipError {
    #[error("Too many updates in message: {count} (max: {max})")]
    TooManyUpdates { count: usize, max: usize },

    #[error("Message size {size} exceeds maximum {max}")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Serialization error: {source}")]
    SerializationError { source: Box<bincode::ErrorKind> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_update(id: &str, _incarnation_val: u64) -> NodeUpdate {
        NodeUpdate {
            node_id: NodeId::new(id),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            tags: HashMap::new(),
        }
    }

    #[test]
    fn test_message_with_max_updates() {
        let updates: Vec<NodeUpdate> = (0..MAX_UPDATES_PER_MESSAGE)
            .map(|i| create_test_update(&format!("node-{}", i), i as u64))
            .collect();

        let msg = GossipMessage::new(updates);
        assert!(msg.check_size().is_ok());
    }

    #[test]
    fn test_message_exceeds_max_updates() {
        let updates: Vec<NodeUpdate> = (0..MAX_UPDATES_PER_MESSAGE + 1)
            .map(|i| create_test_update(&format!("node-{}", i), i as u64))
            .collect();

        let msg = GossipMessage::new(updates);
        let result = msg.check_size();

        assert!(result.is_err());
        match result {
            Err(GossipError::TooManyUpdates { count, max }) => {
                assert_eq!(count, MAX_UPDATES_PER_MESSAGE + 1);
                assert_eq!(max, MAX_UPDATES_PER_MESSAGE);
            }
            _ => panic!("Expected TooManyUpdates error"),
        }
    }

    #[test]
    fn test_message_size_check() {
        let mut tags = HashMap::new();
        for i in 0..100 {
            tags.insert(format!("key-{}", i), format!("value-{}", i));
        }

        let mut update = create_test_update("large-node", 1);
        update.tags = tags;

        let updates = vec![update; 20];
        let msg = GossipMessage::new(updates);

        let serialized = bincode::serialize(&msg).unwrap();
        if serialized.len() > MAX_MESSAGE_SIZE {
            assert!(msg.check_size().is_err());
        } else {
            assert!(msg.check_size().is_ok());
        }
    }

    #[test]
    fn test_empty_message() {
        let msg = GossipMessage::new(vec![]);
        assert!(msg.check_size().is_ok());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
    }

    #[test]
    fn test_node_id() {
        let id1 = NodeId::new("node-1");
        let id2 = NodeId::new("node-1".to_string());

        assert_eq!(id1.as_str(), "node-1");
        assert_eq!(id1, id2);
    }
}
