use crate::cluster::gossip::{NodeId, NodeState};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incarnation(u64);

#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub node_id: NodeId,
    pub addr: SocketAddr,
    pub incarnation: Incarnation,
    pub state: NodeState,
    pub last_seen: Instant,
    pub tags: HashMap<String, String>,
}

impl Incarnation {
    pub fn initial() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;
        Self(timestamp)
    }

    pub fn from_value(value: u64) -> Self {
        Self(value)
    }

    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }

    pub fn compare(&self, other: &Incarnation) -> Ordering {
        const HALF_MAX: u64 = u64::MAX / 2;

        let diff = self.0.wrapping_sub(other.0);

        if diff == 0 {
            Ordering::Equal
        } else if diff < HALF_MAX {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

pub fn resolve_conflict<'a>(a: &'a NodeStatus, b: &'a NodeStatus) -> &'a NodeStatus {
    match a.incarnation.compare(&b.incarnation) {
        Ordering::Greater => a,
        Ordering::Less => b,
        Ordering::Equal => {
            if a.node_id > b.node_id {
                a
            } else {
                b
            }
        }
    }
}

impl PartialOrd for Incarnation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(std::cmp::Ord::cmp(self, other))
    }
}

impl Ord for Incarnation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_incarnation() {
        let inc1 = Incarnation::initial();
        let inc2 = Incarnation::initial();

        assert!(inc2.0 >= inc1.0);
    }

    #[test]
    fn test_increment() {
        let mut inc = Incarnation(100);
        inc.increment();
        assert_eq!(inc.0, 101);

        let mut inc_max = Incarnation(u64::MAX);
        inc_max.increment();
        assert_eq!(inc_max.0, 0);
    }

    #[test]
    fn test_wraparound_comparison() {
        let near_max = Incarnation(u64::MAX - 10);
        let near_zero = Incarnation(10);

        assert_eq!(near_zero.compare(&near_max), Ordering::Greater);
        assert_eq!(near_max.compare(&near_zero), Ordering::Less);
    }

    #[test]
    fn test_wraparound_edge_case() {
        let max_val = Incarnation(u64::MAX);
        let zero_val = Incarnation(0);
        let one_val = Incarnation(1);

        assert_eq!(zero_val.compare(&max_val), Ordering::Greater);
        assert_eq!(one_val.compare(&max_val), Ordering::Greater);
        assert_eq!(max_val.compare(&zero_val), Ordering::Less);
    }

    #[test]
    fn test_equal_incarnations() {
        let inc1 = Incarnation(500);
        let inc2 = Incarnation(500);

        assert_eq!(inc1.compare(&inc2), Ordering::Equal);
    }

    #[test]
    fn test_normal_comparison() {
        let inc1 = Incarnation(100);
        let inc2 = Incarnation(200);

        assert_eq!(inc1.compare(&inc2), Ordering::Less);
        assert_eq!(inc2.compare(&inc1), Ordering::Greater);
    }

    #[test]
    fn test_half_max_boundary() {
        let base = Incarnation(1000);
        let just_before_half = Incarnation(1000 + (u64::MAX / 2) - 1);
        let just_after_half = Incarnation(1000 + (u64::MAX / 2) + 1);

        assert_eq!(just_before_half.compare(&base), Ordering::Greater);
        assert_eq!(just_after_half.compare(&base), Ordering::Less);
    }

    #[test]
    fn test_serialization() {
        let inc = Incarnation(12345);
        let serialized = bincode::serialize(&inc).unwrap();
        let deserialized: Incarnation = bincode::deserialize(&serialized).unwrap();

        assert_eq!(inc, deserialized);
    }

    #[test]
    fn test_ord_trait() {
        let inc1 = Incarnation(100);
        let inc2 = Incarnation(200);

        assert!(inc2 > inc1);
        assert!(inc1 < inc2);
    }

    #[test]
    fn test_wraparound_with_ord() {
        let near_max = Incarnation(u64::MAX - 10);
        let near_zero = Incarnation(10);

        assert!(near_zero > near_max);
        assert!(near_max < near_zero);
    }

    #[test]
    fn test_resolve_conflict_higher_incarnation() {
        let status_a = NodeStatus {
            node_id: NodeId::new("node-a"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            incarnation: Incarnation(100),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        let status_b = NodeStatus {
            node_id: NodeId::new("node-b"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            incarnation: Incarnation(50),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        let winner = super::resolve_conflict(&status_a, &status_b);
        assert_eq!(winner.node_id.as_str(), "node-a");
    }

    #[test]
    fn test_resolve_conflict_equal_incarnation_uses_node_id() {
        let status_a = NodeStatus {
            node_id: NodeId::new("node-b"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            incarnation: Incarnation(100),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        let status_b = NodeStatus {
            node_id: NodeId::new("node-a"),
            addr: "127.0.0.1:8002".parse().unwrap(),
            incarnation: Incarnation(100),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        let winner = super::resolve_conflict(&status_a, &status_b);
        assert_eq!(winner.node_id.as_str(), "node-b");
    }

    #[test]
    fn test_resolve_conflict_deterministic() {
        let status_a = NodeStatus {
            node_id: NodeId::new("node-a"),
            addr: "127.0.0.1:8001".parse().unwrap(),
            incarnation: Incarnation(100),
            state: NodeState::Alive,
            last_seen: Instant::now(),
            tags: HashMap::new(),
        };

        let status_b = status_a.clone();

        let winner1 = super::resolve_conflict(&status_a, &status_b);
        let winner2 = super::resolve_conflict(&status_b, &status_a);

        assert_eq!(winner1.node_id, winner2.node_id);
    }
}
