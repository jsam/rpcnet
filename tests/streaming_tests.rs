#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
use rpcnet::{RpcClient, RpcConfig, RpcServer};
use std::time::Duration;
use tokio::time::timeout;

/// Helper function to create test configuration
fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
}

/// Helper function to start a simple test server
async fn start_test_server() -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
    let mut server = RpcServer::new(test_config());

    // Register regular RPC handlers (these work fine)
    server
        .register("simple_echo", |params| async move { Ok(params) })
        .await;

    server
        .register("multiply", |params| async move {
            if let Ok(number) = bincode::deserialize::<i32>(&params) {
                let result = number * 2;
                bincode::serialize(&result).map_err(rpcnet::RpcError::SerializationError)
            } else {
                Err(rpcnet::RpcError::SerializationError(bincode::Error::new(
                    bincode::ErrorKind::Custom("Invalid input".to_string()),
                )))
            }
        })
        .await;

    // Register streaming handlers (but we won't test the complex bidirectional ones)
    server
        .register_streaming("simple_stream", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                for i in 0..3 {
                    yield Ok(format!("stream_message_{}", i).into_bytes());
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                >
        })
        .await;

    // Start server
    let quic_server = server.bind()?;
    let addr = quic_server.local_addr()?;

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone
            .start(quic_server)
            .await
            .expect("Server should start");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(addr)
}

#[tokio::test]
async fn test_regular_rpc_still_works() {
    let server_addr = start_test_server().await.expect("Server should start");

    let client = RpcClient::connect(server_addr, test_config())
        .await
        .expect("Client should connect");

    // Test regular RPC still works
    let response = client
        .call("simple_echo", b"hello".to_vec())
        .await
        .expect("Regular RPC should work");

    assert_eq!(response, b"hello");
}

#[tokio::test]
async fn test_regular_rpc_with_serialization() {
    let server_addr = start_test_server().await.expect("Server should start");

    let client = RpcClient::connect(server_addr, test_config())
        .await
        .expect("Client should connect");

    // Test regular RPC with serialization
    let number = 21;
    let request = bincode::serialize(&number).expect("Serialization should work");
    let response = client
        .call("multiply", request)
        .await
        .expect("Regular RPC should work");

    let result: i32 = bincode::deserialize(&response).expect("Deserialization should work");
    assert_eq!(result, 42);
}

#[tokio::test]
async fn test_streaming_server_registration() {
    let server_addr = start_test_server().await.expect("Server should start");

    // Just test that server starts successfully with streaming handlers registered
    let client = RpcClient::connect(server_addr, test_config())
        .await
        .expect("Client should connect even with streaming handlers");

    // Test that regular RPC still works when streaming handlers are present
    let response = client
        .call("simple_echo", b"test".to_vec())
        .await
        .expect("Regular RPC should work with streaming handlers present");

    assert_eq!(response, b"test");
}

#[tokio::test]
async fn test_concurrent_regular_rpc() {
    let server_addr = start_test_server().await.expect("Server should start");

    // Test multiple concurrent regular RPC calls
    let mut tasks = Vec::new();

    for i in 0..5 {
        let task = tokio::spawn(async move {
            let client = RpcClient::connect(server_addr, test_config())
                .await
                .expect("Client should connect");

            let message = format!("message_{}", i);
            let response = client
                .call("simple_echo", message.as_bytes().to_vec())
                .await
                .expect("Regular RPC should work");

            String::from_utf8(response).expect("Should be valid UTF8")
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete with timeout
    let results = timeout(Duration::from_secs(10), futures::future::join_all(tasks))
        .await
        .expect("All tasks should complete within timeout");

    // Verify results
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(message) => assert_eq!(message, &format!("message_{}", i)),
            Err(e) => panic!("Task {} failed: {:?}", i, e),
        }
    }
}

#[tokio::test]
async fn test_streaming_handler_compilation() {
    // Test that we can create streaming handlers without runtime issues
    let server = RpcServer::new(test_config());

    // This should complete quickly without hanging
    let registration_result = timeout(Duration::from_secs(5), async {
        server
            .register_streaming("test_handler", |_request_stream| async move {
                Box::pin(async_stream::stream! {
                    yield Ok(b"test_response".to_vec());
                })
                    as std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                    >
            })
            .await;
        "completed"
    })
    .await;

    assert!(
        registration_result.is_ok(),
        "Streaming handler registration should not hang"
    );
    assert_eq!(registration_result.unwrap(), "completed");
}

#[tokio::test]
async fn test_server_bind_with_streaming() {
    // Test that server can bind successfully with streaming handlers
    let mut server = RpcServer::new(test_config());

    server
        .register_streaming("bind_test", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                yield Ok(b"bind_successful".to_vec());
            })
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                >
        })
        .await;

    let bind_result = server.bind();
    assert!(
        bind_result.is_ok(),
        "Server should bind successfully with streaming handlers"
    );
}

// Test basic performance - regular RPC should be fast
#[tokio::test]
async fn test_regular_rpc_performance() {
    let server_addr = start_test_server().await.expect("Server should start");

    let client = RpcClient::connect(server_addr, test_config())
        .await
        .expect("Client should connect");

    // Test 100 rapid requests complete quickly
    let start = std::time::Instant::now();

    for i in 0..100 {
        let message = format!("msg_{}", i);
        let _response = client
            .call("simple_echo", message.into_bytes())
            .await
            .expect("RPC should succeed");
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "100 RPC calls should complete within 10 seconds"
    );
}
