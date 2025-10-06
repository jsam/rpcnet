// Unit tests specifically targeting uncovered code paths
// These tests directly exercise the methods without requiring full networking

use futures::StreamExt;
use rpcnet::{RpcConfig, RpcError, RpcServer};
use std::time::Duration;

fn create_basic_config() -> RpcConfig {
    RpcConfig::new("test_cert.pem", "127.0.0.1:0")
        .with_key_path("test_key.pem")
        .with_server_name("localhost")
}

#[tokio::test]
async fn test_rpc_server_new() {
    // Test RpcServer::new constructor - this should be covered
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Verify server was created successfully (we can't access private fields)
    // Just ensure we can use the server for registration
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 0);
    drop(handlers);

    let streaming_handlers = server.streaming_handlers.read().await;
    assert_eq!(streaming_handlers.len(), 0);
}

#[tokio::test]
async fn test_register_basic_handler() {
    // Test RpcServer::register method
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Register a simple handler
    server
        .register("test_method", |params| async move {
            Ok(params) // Echo the parameters
        })
        .await;

    // Verify handler was registered
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 1);
    assert!(handlers.contains_key("test_method"));
}

#[tokio::test]
async fn test_register_streaming_handler() {
    // Test RpcServer::register_streaming method
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Register a streaming handler
    server
        .register_streaming("stream_method", |mut request_stream| async move {
            Box::pin(async_stream::stream! {
                while let Some(data) = request_stream.next().await {
                    yield Ok(data);
                }
            })
        })
        .await;

    // Verify streaming handler was registered
    let streaming_handlers = server.streaming_handlers.read().await;
    assert_eq!(streaming_handlers.len(), 1);
    assert!(streaming_handlers.contains_key("stream_method"));
}

#[tokio::test]
async fn test_rpc_request_methods() {
    // Test RpcRequest methods for better coverage
    use rpcnet::RpcRequest;

    let request = RpcRequest::new(12345, "test_method".to_string(), vec![1, 2, 3]);

    // Test all accessor methods
    assert_eq!(request.id(), 12345);
    assert_eq!(request.method(), "test_method");
    assert_eq!(request.params(), &vec![1, 2, 3]);
}

#[tokio::test]
async fn test_rpc_response_methods() {
    // Test RpcResponse methods for better coverage
    use rpcnet::RpcResponse;

    // Test successful response
    let success_response = RpcResponse::from_result(123, Ok(vec![4, 5, 6]));
    assert_eq!(success_response.id(), 123);
    assert_eq!(success_response.result(), Some(&vec![4, 5, 6]));
    assert_eq!(success_response.error(), None);

    // Test error response
    let error_response =
        RpcResponse::from_result(456, Err(RpcError::StreamError("test error".to_string())));
    assert_eq!(error_response.id(), 456);
    assert_eq!(error_response.result(), None);
    assert!(error_response.error().is_some());
}

#[tokio::test]
async fn test_rpc_config_builder_methods() {
    // Test RpcConfig builder methods
    let config = RpcConfig::new("cert.pem", "localhost:8080")
        .with_key_path("key.pem")
        .with_server_name("test-server")
        .with_keep_alive_interval(Duration::from_secs(30));

    // The config should be created successfully
    // We can't easily test internal state without making fields public,
    // but we can ensure the builder pattern works
    drop(config);
}

#[tokio::test]
async fn test_error_variants() {
    // Test different RpcError variants to improve coverage
    let stream_error = RpcError::StreamError("stream failed".to_string());
    let serialization_error = RpcError::SerializationError(bincode::Error::new(
        bincode::ErrorKind::InvalidBoolEncoding(101),
    ));

    // Test Debug formatting
    let _stream_debug = format!("{:?}", stream_error);
    let _ser_debug = format!("{:?}", serialization_error);

    // Test Display formatting
    let _stream_display = format!("{}", stream_error);
    let _ser_display = format!("{}", serialization_error);
}

#[tokio::test]
async fn test_multiple_handler_registration() {
    // Test registering multiple handlers
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Register multiple handlers
    server
        .register("method1", |_| async move { Ok(vec![1]) })
        .await;
    server
        .register("method2", |_| async move { Ok(vec![2]) })
        .await;
    server
        .register("method3", |_| async move { Ok(vec![3]) })
        .await;

    // Register multiple streaming handlers
    server
        .register_streaming("stream1", |mut req_stream| async move {
            Box::pin(async_stream::stream! {
                while let Some(data) = req_stream.next().await {
                    yield Ok(data);
                }
            })
        })
        .await;

    server
        .register_streaming("stream2", |mut req_stream| async move {
            Box::pin(async_stream::stream! {
                while let Some(data) = req_stream.next().await {
                    yield Ok(data);
                }
            })
        })
        .await;

    // Verify all handlers were registered
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 3);
    assert!(handlers.contains_key("method1"));
    assert!(handlers.contains_key("method2"));
    assert!(handlers.contains_key("method3"));
    drop(handlers);

    let streaming_handlers = server.streaming_handlers.read().await;
    assert_eq!(streaming_handlers.len(), 2);
    assert!(streaming_handlers.contains_key("stream1"));
    assert!(streaming_handlers.contains_key("stream2"));
}

#[tokio::test]
async fn test_handler_replacement() {
    // Test that registering a handler with the same name replaces the old one
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Register handler
    server
        .register("replaceable", |_| async move { Ok(vec![1]) })
        .await;

    // Verify initial registration
    {
        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 1);
    }

    // Replace handler with same name
    server
        .register("replaceable", |_| async move { Ok(vec![2]) })
        .await;

    // Should still have only one handler
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 1);
    assert!(handlers.contains_key("replaceable"));
}

#[tokio::test]
async fn test_empty_method_names() {
    // Test edge cases with method names
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Register handler with empty name (edge case)
    server.register("", |_| async move { Ok(vec![]) }).await;

    // Should still work
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 1);
    assert!(handlers.contains_key(""));
}

#[tokio::test]
async fn test_large_method_names() {
    // Test with very long method names
    let config = create_basic_config();
    let server = RpcServer::new(config);

    let long_name = "a".repeat(1000);
    server
        .register(&long_name, |_| async move { Ok(vec![]) })
        .await;

    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 1);
    assert!(handlers.contains_key(&long_name));
}

#[tokio::test]
async fn test_handler_with_different_return_types() {
    // Test handlers that return different types of results
    let config = create_basic_config();
    let server = RpcServer::new(config);

    // Handler that returns success
    server
        .register("success", |_| async move { Ok(vec![1, 2, 3]) })
        .await;

    // Handler that returns error
    server
        .register("error", |_| async move {
            Err(RpcError::StreamError("intentional error".to_string()))
        })
        .await;

    // Handler that returns empty data
    server
        .register("empty", |_| async move { Ok(vec![]) })
        .await;

    // Handler that returns large data
    server
        .register("large", |_| async move { Ok(vec![0u8; 10000]) })
        .await;

    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 4);
}
