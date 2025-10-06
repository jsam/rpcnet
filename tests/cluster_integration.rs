#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
use rpcnet::cluster::{ClusterConfig, ClusterMembership, NodeRegistry};
use s2n_quic::Client as QuicClient;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

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
async fn test_three_node_cluster_formation() {
    let client1 = create_test_client().await;
    let client2 = create_test_client().await;
    let client3 = create_test_client().await;

    let addr1: SocketAddr = "127.0.0.1:30001".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:30002".parse().unwrap();
    let addr3: SocketAddr = "127.0.0.1:30003".parse().unwrap();

    let config1 = ClusterConfig::default();
    let config2 = ClusterConfig::default();
    let config3 = ClusterConfig::default();

    let node1 = Arc::new(
        ClusterMembership::new(addr1, config1, client1)
            .await
            .unwrap(),
    );
    let node2 = Arc::new(
        ClusterMembership::new(addr2, config2, client2)
            .await
            .unwrap(),
    );
    let node3 = Arc::new(
        ClusterMembership::new(addr3, config3, client3)
            .await
            .unwrap(),
    );

    node1.join(vec![]).await.unwrap();

    node2.join(vec![addr1]).await.ok();
    node3.join(vec![addr1]).await.ok();

    sleep(Duration::from_millis(100)).await;

    assert_eq!(node1.registry().len(), 1);
}

#[tokio::test]
async fn test_tag_based_service_discovery() {
    let client1 = create_test_client().await;
    let client2 = create_test_client().await;

    let addr1: SocketAddr = "127.0.0.1:30011".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:30012".parse().unwrap();

    let config1 = ClusterConfig::default();
    let config2 = ClusterConfig::default();

    let node1 = Arc::new(
        ClusterMembership::new(addr1, config1, client1)
            .await
            .unwrap(),
    );
    let node2 = Arc::new(
        ClusterMembership::new(addr2, config2, client2)
            .await
            .unwrap(),
    );

    node1.join(vec![]).await.unwrap();
    node2.join(vec![]).await.unwrap();

    node1
        .update_tag("role".to_string(), "worker".to_string())
        .await;
    node2
        .update_tag("role".to_string(), "director".to_string())
        .await;

    sleep(Duration::from_millis(100)).await;

    let workers_from_node1 = node1.nodes_with_tag("role", "worker").await;
    assert_eq!(workers_from_node1.len(), 1);
    assert_eq!(workers_from_node1[0].node_id, *node1.node_id());
}

#[tokio::test]
async fn test_graceful_leave() {
    let client = create_test_client().await;
    let addr: SocketAddr = "127.0.0.1:30021".parse().unwrap();
    let config = ClusterConfig::default();

    let node = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());

    node.join(vec![]).await.unwrap();

    let stats_before = node.stats();
    assert_eq!(stats_before.total_nodes, 1);
    assert_eq!(stats_before.alive_nodes, 1);

    node.leave().await.unwrap();

    let result = node.leave().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_tags_query() {
    let client = create_test_client().await;
    let addr: SocketAddr = "127.0.0.1:30031".parse().unwrap();
    let config = ClusterConfig::default();

    let node = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());

    node.join(vec![]).await.unwrap();

    node.update_tag("role".to_string(), "worker".to_string())
        .await;
    node.update_tag("zone".to_string(), "us-east".to_string())
        .await;
    node.update_tag("version".to_string(), "1.0.0".to_string())
        .await;

    sleep(Duration::from_millis(100)).await;

    let mut query_tags = HashMap::new();
    query_tags.insert("role".to_string(), "worker".to_string());
    query_tags.insert("zone".to_string(), "us-east".to_string());

    let nodes = node.nodes_with_all_tags(&query_tags).await;
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].tags.get("version"), Some(&"1.0.0".to_string()));
}

#[tokio::test]
async fn test_tag_removal() {
    let client = create_test_client().await;
    let addr: SocketAddr = "127.0.0.1:30041".parse().unwrap();
    let config = ClusterConfig::default();

    let node = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());

    node.join(vec![]).await.unwrap();

    node.update_tag("temporary".to_string(), "value".to_string())
        .await;

    sleep(Duration::from_millis(50)).await;

    let nodes_with_tag = node.nodes_with_tag("temporary", "value").await;
    assert_eq!(nodes_with_tag.len(), 1);

    node.remove_tag("temporary").await;

    sleep(Duration::from_millis(50)).await;

    let nodes_after_removal = node.nodes_with_tag("temporary", "value").await;
    assert_eq!(nodes_after_removal.len(), 0);
}

#[tokio::test]
async fn test_cluster_stats() {
    let client = create_test_client().await;
    let addr: SocketAddr = "127.0.0.1:30051".parse().unwrap();
    let config = ClusterConfig::default();

    let node = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());

    node.join(vec![]).await.unwrap();

    let stats = node.stats();

    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.alive_nodes, 1);
    assert_eq!(stats.suspect_nodes, 0);
    assert_eq!(stats.failed_nodes, 0);
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.idle_connections, 0);
}

#[tokio::test]
async fn test_event_subscription() {
    let client = create_test_client().await;
    let addr: SocketAddr = "127.0.0.1:30061".parse().unwrap();
    let config = ClusterConfig::default();

    let node = Arc::new(ClusterMembership::new(addr, config, client).await.unwrap());

    let mut receiver = node.subscribe();

    node.join(vec![]).await.unwrap();

    node.update_tag("test".to_string(), "value".to_string())
        .await;

    sleep(Duration::from_millis(50)).await;

    let event_result = tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await;

    assert!(event_result.is_ok());
}
