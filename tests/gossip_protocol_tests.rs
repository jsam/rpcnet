#![allow(clippy::all)]
#![allow(warnings)]

//! Comprehensive tests for SWIM gossip protocol implementation
//!
//! This test suite focuses on the low-level gossip protocol mechanisms:
//! - Message serialization and deserialization
//! - Ping/Ack/PingReq message flow
//! - Gossip queue behavior
//! - Update propagation and redundancy control
//! - Configuration validation

use rpcnet::cluster::gossip::config::GossipConfig;
use rpcnet::cluster::gossip::message::{
    GossipMessage, NodeId, NodeState, NodeUpdate, Priority, MAX_MESSAGE_SIZE,
    MAX_UPDATES_PER_MESSAGE,
};
use rpcnet::cluster::gossip::queue::GossipQueue;
use rpcnet::cluster::gossip::swim::SwimMessage;
use rpcnet::cluster::incarnation::Incarnation;
use std::collections::HashMap;
use std::time::Duration;

//------------------------------------------------------------------------------
// GossipConfig Tests
//------------------------------------------------------------------------------

#[test]
fn test_gossip_config_default_values() {
    let config = GossipConfig::default();

    assert_eq!(config.protocol_period, Duration::from_secs(1));
    assert_eq!(config.indirect_ping_count, 3);
    assert_eq!(config.ack_timeout, Duration::from_millis(500));
    assert_eq!(config.indirect_timeout, Duration::from_millis(1000));
}

#[test]
fn test_gossip_config_builder() {
    let config = GossipConfig::new()
        .with_protocol_period(Duration::from_millis(500))
        .with_indirect_ping_count(5)
        .with_ack_timeout(Duration::from_millis(200))
        .with_indirect_timeout(Duration::from_millis(800));

    assert_eq!(config.protocol_period, Duration::from_millis(500));
    assert_eq!(config.indirect_ping_count, 5);
    assert_eq!(config.ack_timeout, Duration::from_millis(200));
    assert_eq!(config.indirect_timeout, Duration::from_millis(800));
}

#[test]
fn test_gossip_config_chaining() {
    // Test that builder pattern allows chaining
    let config = GossipConfig::default()
        .with_protocol_period(Duration::from_millis(100))
        .with_ack_timeout(Duration::from_millis(50));

    assert_eq!(config.protocol_period, Duration::from_millis(100));
    assert_eq!(config.ack_timeout, Duration::from_millis(50));
    // Other values should remain at defaults
    assert_eq!(config.indirect_ping_count, 3);
}

//------------------------------------------------------------------------------
// SwimMessage Tests (Extended)
//------------------------------------------------------------------------------

#[test]
fn test_swim_ping_message_structure() {
    let ping = SwimMessage::Ping {
        from: NodeId::new("node-1"),
        from_addr: "127.0.0.1:8000".parse().unwrap(),
        updates: vec![],
        seq: 42,
    };

    assert_eq!(ping.seq(), 42);
    assert_eq!(ping.from_node().as_str(), "node-1");
    assert_eq!(ping.updates().len(), 0);
}

#[test]
fn test_swim_ack_message_structure() {
    let ack = SwimMessage::Ack {
        from: NodeId::new("node-2"),
        to: NodeId::new("node-1"),
        updates: vec![],
        seq: 100,
    };

    assert_eq!(ack.seq(), 100);
    assert_eq!(ack.from_node().as_str(), "node-2");
}

#[test]
fn test_swim_ping_req_message_structure() {
    let ping_req = SwimMessage::PingReq {
        from: NodeId::new("node-1"),
        target: "127.0.0.1:8002".parse().unwrap(),
        target_id: NodeId::new("node-3"),
        updates: vec![],
        seq: 200,
    };

    assert_eq!(ping_req.seq(), 200);
    assert_eq!(ping_req.from_node().as_str(), "node-1");
}

#[test]
fn test_swim_message_with_updates() {
    let update = NodeUpdate {
        node_id: NodeId::new("node-4"),
        addr: "127.0.0.1:8004".parse().unwrap(),
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        tags: HashMap::new(),
    };

    let ping = SwimMessage::Ping {
        from: NodeId::new("node-1"),
        from_addr: "127.0.0.1:8000".parse().unwrap(),
        updates: vec![update.clone()],
        seq: 1,
    };

    assert_eq!(ping.updates().len(), 1);
    assert_eq!(ping.updates()[0].node_id.as_str(), "node-4");
}

#[test]
fn test_swim_message_serialization_roundtrip() {
    let original = SwimMessage::Ping {
        from: NodeId::new("test-node"),
        from_addr: "192.168.1.100:9000".parse().unwrap(),
        updates: vec![],
        seq: 12345,
    };

    let serialized = original.serialize().unwrap();
    let deserialized = SwimMessage::deserialize(&serialized).unwrap();

    assert_eq!(original.seq(), deserialized.seq());
    assert_eq!(original.from_node(), deserialized.from_node());
}

//------------------------------------------------------------------------------
// GossipQueue Advanced Tests
//------------------------------------------------------------------------------

#[test]
fn test_gossip_queue_priority_across_multiple_nodes() {
    let queue = GossipQueue::new(10);

    // Add multiple updates for different nodes with different priorities
    for i in 0..5 {
        let update = create_test_update(&format!("low-{}", i));
        queue.enqueue(update, Priority::Low);
    }

    for i in 0..3 {
        let update = create_test_update(&format!("high-{}", i));
        queue.enqueue(update, Priority::High);
    }

    for i in 0..2 {
        let update = create_test_update(&format!("critical-{}", i));
        queue.enqueue(update, Priority::Critical);
    }

    let selected = queue.select_updates();

    // Verify critical comes first
    assert!(selected[0].node_id.as_str().starts_with("critical"));
    assert!(selected[1].node_id.as_str().starts_with("critical"));

    // Then high priority
    assert!(selected[2].node_id.as_str().starts_with("high"));
}

#[test]
fn test_gossip_queue_logarithmic_redundancy() {
    // Test that gossip rounds follow log(N) * 3 formula
    let test_cases = vec![
        (10, 12),   // log2(10) = 3.32, ceil = 4, * 3 = 12
        (100, 21), // log2(100) = 6.64, ceil = 7, * 3 = 21
        (1000, 30), // log2(1000) = 9.97, ceil = 10, * 3 = 30
    ];

    for (cluster_size, expected_max_rounds) in test_cases {
        let queue = GossipQueue::new(cluster_size);
        let node_id = NodeId::new("test-node");

        queue.enqueue(create_test_update("test-node"), Priority::High);

        // Mark as sent up to max_rounds - 1
        for _ in 0..(expected_max_rounds - 1) {
            assert!(!queue.should_stop_gossiping(&node_id));
            queue.mark_sent(&node_id);
        }

        // One more should reach the limit
        queue.mark_sent(&node_id);

        // Now it should stop
        assert!(queue.should_stop_gossiping(&node_id));
    }
}

#[test]
fn test_gossip_queue_dynamic_cluster_size() {
    let queue = GossipQueue::new(10);
    let node_id = NodeId::new("test");

    // With cluster size 10, max_rounds = 12
    for _ in 0..9 {
        queue.mark_sent(&node_id);
    }
    assert!(!queue.should_stop_gossiping(&node_id));

    // Increase cluster size to 100 (max_rounds = 21)
    queue.update_cluster_size(100);
    queue.clear_seen_counts(); // Reset for new calculation

    for _ in 0..20 {
        queue.mark_sent(&node_id);
    }
    assert!(!queue.should_stop_gossiping(&node_id));

    queue.mark_sent(&node_id);
    assert!(queue.should_stop_gossiping(&node_id));
}

#[test]
fn test_gossip_queue_remove_after_select() {
    let queue = GossipQueue::new(10);

    queue.enqueue(create_test_update("node-1"), Priority::High);
    queue.enqueue(create_test_update("node-2"), Priority::High);

    assert_eq!(queue.len(), 2);

    let selected = queue.select_updates();
    assert_eq!(selected.len(), 2);

    // After selection, queue should be empty (updates removed)
    assert_eq!(queue.len(), 0);
}

#[test]
fn test_gossip_queue_multiple_selections() {
    let queue = GossipQueue::new(10);

    // Add 30 updates
    for i in 0..30 {
        queue.enqueue(create_test_update(&format!("node-{}", i)), Priority::Medium);
    }

    // First selection gets MAX_UPDATES_PER_MESSAGE
    let first_batch = queue.select_updates();
    assert_eq!(first_batch.len(), MAX_UPDATES_PER_MESSAGE);

    // Remaining updates
    let remaining = queue.len();
    assert_eq!(remaining, 30 - MAX_UPDATES_PER_MESSAGE);

    // Second selection gets the rest
    let second_batch = queue.select_updates();
    assert_eq!(second_batch.len(), remaining);

    assert!(queue.is_empty());
}

//------------------------------------------------------------------------------
// NodeUpdate and NodeState Tests
//------------------------------------------------------------------------------

#[test]
fn test_node_state_transitions() {
    // Valid state transitions in SWIM:
    // Alive -> Suspect -> Failed
    // Any state -> Left

    let states = vec![
        NodeState::Alive,
        NodeState::Suspect,
        NodeState::Failed,
        NodeState::Left,
    ];

    for state in states {
        let update = NodeUpdate {
            node_id: NodeId::new("test"),
            addr: "127.0.0.1:8000".parse().unwrap(),
            incarnation: Incarnation::initial(),
            state,
            tags: HashMap::new(),
        };

        // Verify we can create updates with all states
        assert_eq!(update.state, state);
    }
}

#[test]
fn test_node_update_with_tags() {
    let mut tags = HashMap::new();
    tags.insert("role".to_string(), "worker".to_string());
    tags.insert("zone".to_string(), "us-east-1".to_string());
    tags.insert("version".to_string(), "1.2.3".to_string());

    let update = NodeUpdate {
        node_id: NodeId::new("worker-1"),
        addr: "10.0.1.5:8080".parse().unwrap(),
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        tags: tags.clone(),
    };

    assert_eq!(update.tags.len(), 3);
    assert_eq!(update.tags.get("role"), Some(&"worker".to_string()));
    assert_eq!(update.tags.get("zone"), Some(&"us-east-1".to_string()));
}

#[test]
fn test_node_update_serialization_with_tags() {
    let mut tags = HashMap::new();
    tags.insert("key".to_string(), "value".to_string());

    let update = NodeUpdate {
        node_id: NodeId::new("node-1"),
        addr: "127.0.0.1:8000".parse().unwrap(),
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        tags,
    };

    let msg = GossipMessage::new(vec![update.clone()]);
    let serialized = rmp_serde::to_vec(&msg).unwrap();
    let deserialized: GossipMessage = rmp_serde::from_slice(&serialized).unwrap();

    assert_eq!(deserialized.updates.len(), 1);
    assert_eq!(deserialized.updates[0].tags.get("key"), Some(&"value".to_string()));
}

//------------------------------------------------------------------------------
// GossipMessage Size and Limit Tests
//------------------------------------------------------------------------------

#[test]
fn test_gossip_message_exact_max_size() {
    // Create a message with exactly MAX_UPDATES_PER_MESSAGE updates
    let updates: Vec<NodeUpdate> = (0..MAX_UPDATES_PER_MESSAGE)
        .map(|i| create_test_update(&format!("node-{}", i)))
        .collect();

    let msg = GossipMessage::new(updates);
    assert!(msg.check_size().is_ok());
}

#[test]
fn test_gossip_message_oversized_payload() {
    // Create an update with huge tags to exceed MAX_MESSAGE_SIZE
    let mut huge_tags = HashMap::new();
    for i in 0..1000 {
        huge_tags.insert(
            format!("very_long_key_name_{}", i),
            format!("very_long_value_that_takes_up_space_{}", i),
        );
    }

    let update = NodeUpdate {
        node_id: NodeId::new("huge-node"),
        addr: "127.0.0.1:8000".parse().unwrap(),
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        tags: huge_tags,
    };

    // Even a single huge update might exceed MAX_MESSAGE_SIZE
    let msg = GossipMessage::new(vec![update]);
    let serialized = rmp_serde::to_vec(&msg).unwrap();

    if serialized.len() > MAX_MESSAGE_SIZE {
        assert!(msg.check_size().is_err());
    }
}

#[test]
fn test_empty_gossip_message() {
    let msg = GossipMessage::new(vec![]);
    assert!(msg.check_size().is_ok());

    let serialized = rmp_serde::to_vec(&msg).unwrap();
    assert!(serialized.len() < MAX_MESSAGE_SIZE);
}

//------------------------------------------------------------------------------
// Priority Tests
//------------------------------------------------------------------------------

#[test]
fn test_priority_numeric_values() {
    assert_eq!(Priority::Critical as u8, 3);
    assert_eq!(Priority::High as u8, 2);
    assert_eq!(Priority::Medium as u8, 1);
    assert_eq!(Priority::Low as u8, 0);
}

#[test]
fn test_priority_comparison() {
    assert!(Priority::Critical > Priority::High);
    assert!(Priority::High > Priority::Medium);
    assert!(Priority::Medium > Priority::Low);

    assert!(Priority::Critical >= Priority::Critical);
    assert!(Priority::Low <= Priority::Medium);
}

#[test]
fn test_priority_serialization() {
    let priorities = vec![
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ];

    for priority in priorities {
        let serialized = rmp_serde::to_vec(&priority).unwrap();
        let deserialized: Priority = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(priority, deserialized);
    }
}

//------------------------------------------------------------------------------
// NodeId Tests
//------------------------------------------------------------------------------

#[test]
fn test_node_id_creation() {
    let id1 = NodeId::new("node-1");
    let id2 = NodeId::new("node-1".to_string());

    assert_eq!(id1, id2);
    assert_eq!(id1.as_str(), "node-1");
}

#[test]
fn test_node_id_ordering() {
    let id1 = NodeId::new("a");
    let id2 = NodeId::new("b");
    let id3 = NodeId::new("c");

    assert!(id1 < id2);
    assert!(id2 < id3);
    assert!(id1 < id3);
}

#[test]
fn test_node_id_hash_equality() {
    use std::collections::HashSet;

    let id1 = NodeId::new("same");
    let id2 = NodeId::new("same");
    let id3 = NodeId::new("different");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    set.insert(id2.clone());
    set.insert(id3.clone());

    // id1 and id2 are the same, so set should only have 2 entries
    assert_eq!(set.len(), 2);
    assert!(set.contains(&id1));
    assert!(set.contains(&id3));
}

//------------------------------------------------------------------------------
// Edge Cases and Error Conditions
//------------------------------------------------------------------------------

#[test]
fn test_gossip_queue_zero_cluster_size() {
    // Cluster size should be at least 1
    let queue = GossipQueue::new(0);

    // Internal implementation uses max(1, size)
    // For cluster size 1: log2(1) = 0, ceil = 0, * 3 = 0
    // This means should_stop_gossiping will return true immediately
    let node_id = NodeId::new("test");
    queue.enqueue(create_test_update("test"), Priority::High);

    // With cluster size 1, max_rounds = 0, so it stops immediately
    assert!(queue.should_stop_gossiping(&node_id));
}

#[test]
fn test_swim_message_large_sequence_number() {
    let ping = SwimMessage::Ping {
        from: NodeId::new("node-1"),
        from_addr: "127.0.0.1:8000".parse().unwrap(),
        updates: vec![],
        seq: u64::MAX,
    };

    assert_eq!(ping.seq(), u64::MAX);

    // Verify serialization works with max values
    let serialized = ping.serialize().unwrap();
    let deserialized = SwimMessage::deserialize(&serialized).unwrap();
    assert_eq!(deserialized.seq(), u64::MAX);
}

#[test]
fn test_gossip_message_boundary_conditions() {
    // Test with exactly MAX_UPDATES_PER_MESSAGE updates
    let updates: Vec<NodeUpdate> = (0..MAX_UPDATES_PER_MESSAGE)
        .map(|i| create_test_update(&format!("node-{}", i)))
        .collect();

    let msg = GossipMessage::new(updates);
    assert!(msg.check_size().is_ok());

    // Test with one more than max
    let updates: Vec<NodeUpdate> = (0..MAX_UPDATES_PER_MESSAGE + 1)
        .map(|i| create_test_update(&format!("node-{}", i)))
        .collect();

    let msg = GossipMessage::new(updates);
    assert!(msg.check_size().is_err());
}

//------------------------------------------------------------------------------
// Helper Functions
//------------------------------------------------------------------------------

fn create_test_update(id: &str) -> NodeUpdate {
    NodeUpdate {
        node_id: NodeId::new(id),
        addr: "127.0.0.1:8000".parse().unwrap(),
        incarnation: Incarnation::initial(),
        state: NodeState::Alive,
        tags: HashMap::new(),
    }
}
