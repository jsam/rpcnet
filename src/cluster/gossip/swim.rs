use crate::cluster::gossip::{NodeId, NodeUpdate};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwimMessage {
    Ping {
        from: NodeId,
        from_addr: SocketAddr,
        updates: Vec<NodeUpdate>,
        seq: u64,
    },
    Ack {
        from: NodeId,
        to: NodeId,
        updates: Vec<NodeUpdate>,
        seq: u64,
    },
    PingReq {
        from: NodeId,
        target: SocketAddr,
        target_id: NodeId,
        updates: Vec<NodeUpdate>,
        seq: u64,
    },
}

impl SwimMessage {
    pub fn seq(&self) -> u64 {
        match self {
            SwimMessage::Ping { seq, .. } => *seq,
            SwimMessage::Ack { seq, .. } => *seq,
            SwimMessage::PingReq { seq, .. } => *seq,
        }
    }

    pub fn from_node(&self) -> &NodeId {
        match self {
            SwimMessage::Ping { from, .. } => from,
            SwimMessage::Ack { from, .. } => from,
            SwimMessage::PingReq { from, .. } => from,
        }
    }

    pub fn updates(&self) -> &[NodeUpdate] {
        match self {
            SwimMessage::Ping { updates, .. } => updates,
            SwimMessage::Ack { updates, .. } => updates,
            SwimMessage::PingReq { updates, .. } => updates,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Box<bincode::ErrorKind>> {
        bincode::serialize(self)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_serialization() {
        let ping = SwimMessage::Ping {
            from: NodeId::new("node-1"),
            from_addr: "127.0.0.1:8000".parse().unwrap(),
            updates: vec![],
            seq: 42,
        };

        let bytes = ping.serialize().unwrap();
        let deserialized = SwimMessage::deserialize(&bytes).unwrap();

        assert_eq!(ping.seq(), deserialized.seq());
        assert_eq!(ping.from_node().as_str(), "node-1");
    }

    #[test]
    fn test_ack_serialization() {
        let ack = SwimMessage::Ack {
            from: NodeId::new("node-2"),
            to: NodeId::new("node-1"),
            updates: vec![],
            seq: 100,
        };

        let bytes = ack.serialize().unwrap();
        let deserialized = SwimMessage::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.seq(), 100);
    }

    #[test]
    fn test_ping_req_serialization() {
        let ping_req = SwimMessage::PingReq {
            from: NodeId::new("node-1"),
            target: "127.0.0.1:8002".parse().unwrap(),
            target_id: NodeId::new("node-3"),
            updates: vec![],
            seq: 200,
        };

        let bytes = ping_req.serialize().unwrap();
        let deserialized = SwimMessage::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.seq(), 200);
    }
}
