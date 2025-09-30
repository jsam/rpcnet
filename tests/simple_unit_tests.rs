// Simple unit tests that should definitely work and increase coverage

use futures::StreamExt;
use rpcnet::{RpcClient, RpcConfig, RpcError, RpcRequest, RpcResponse, RpcServer};
use std::time::Duration;

#[test]
fn test_rpc_request_creation() {
    // Test RpcRequest::new - this should hit lines 808, 815-816, 822-823, 830-831
    let request = RpcRequest::new(12345, "test_method".to_string(), vec![1, 2, 3, 4]);

    // Test accessor methods
    let id = request.id();
    assert_eq!(id, 12345);

    let method = request.method();
    assert_eq!(method, "test_method");

    let params = request.params();
    assert_eq!(params, &vec![1, 2, 3, 4]);
}

#[test]
fn test_rpc_response_creation() {
    // Test RpcResponse methods - this should hit lines 881-884, 889-890, 896-897, 903-904

    // Test successful response
    let success = RpcResponse::from_result(123, Ok(vec![5, 6, 7]));
    assert_eq!(success.id(), 123);
    assert_eq!(success.result(), Some(&vec![5, 6, 7]));
    assert_eq!(success.error(), None);

    // Test error response
    let error = RpcResponse::from_result(456, Err(RpcError::StreamError("test error".to_string())));
    assert_eq!(error.id(), 456);
    assert_eq!(error.result(), None);
    assert!(error.error().is_some());

    // Test RpcResponse::new directly
    let direct = RpcResponse::new(789, Some(vec![8, 9]), None);
    assert_eq!(direct.id(), 789);
    assert_eq!(direct.result(), Some(&vec![8, 9]));
    assert_eq!(direct.error(), None);
}

#[test]
fn test_rpc_config_creation() {
    // Test RpcConfig::new and builder methods - should hit lines 990, 992, 994-996
    let config = RpcConfig::new("test_cert.pem", "localhost:8080");

    // Test builder methods - should hit lines 1017-1019, 1039-1041, 1068-1070
    let config_with_options = config
        .with_key_path("test_key.pem")
        .with_server_name("test-server")
        .with_keep_alive_interval(Duration::from_secs(30));

    // Just verify we can create the config
    drop(config_with_options);
}

#[tokio::test]
async fn test_rpc_server_creation() {
    // Test RpcServer::new - should hit lines 1203, 1205-1206
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    let server = RpcServer::new(config);

    // Test that we can access the handlers (this proves creation worked)
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 0);
    drop(handlers);

    let streaming_handlers = server.streaming_handlers.read().await;
    assert_eq!(streaming_handlers.len(), 0);
    drop(streaming_handlers);
}

#[tokio::test]
async fn test_register_handler() {
    // Test RpcServer::register - should hit lines 1268, 1273-1277
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    let server = RpcServer::new(config);

    // Register a handler
    server
        .register("echo", |params| async move {
            Ok(params) // Echo back the parameters
        })
        .await;

    // Verify handler was registered
    let handlers = server.handlers.read().await;
    assert_eq!(handlers.len(), 1);
    assert!(handlers.contains_key("echo"));
}

#[tokio::test]
async fn test_register_streaming_handler() {
    // Test RpcServer::register_streaming - should hit lines 1331, 1337-1344
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    let server = RpcServer::new(config);

    // Register a streaming handler
    server
        .register_streaming("stream_echo", |mut request_stream| async move {
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
    assert!(streaming_handlers.contains_key("stream_echo"));
}

#[tokio::test]
async fn test_multiple_handlers() {
    // Test registering multiple handlers
    let config = RpcConfig::new("cert.pem", "127.0.0.1:0");
    let server = RpcServer::new(config);

    // Register multiple regular handlers
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
    drop(handlers);

    let streaming_handlers = server.streaming_handlers.read().await;
    assert_eq!(streaming_handlers.len(), 2);
    drop(streaming_handlers);
}

#[test]
fn test_error_types() {
    // Test RpcError variants to improve coverage
    let stream_error = RpcError::StreamError("connection failed".to_string());
    let ser_error = RpcError::SerializationError(bincode::Error::new(
        bincode::ErrorKind::InvalidBoolEncoding(101),
    ));

    // Test Debug and Display formatting
    let _stream_debug = format!("{:?}", stream_error);
    let _stream_display = format!("{}", stream_error);
    let _ser_debug = format!("{:?}", ser_error);
    let _ser_display = format!("{}", ser_error);

    // Test error matching
    match stream_error {
        RpcError::StreamError(msg) => assert_eq!(msg, "connection failed"),
        _ => panic!("Wrong error type"),
    }

    match ser_error {
        RpcError::SerializationError(_) => {} // Expected
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_request_with_different_data() {
    // Test RpcRequest with different data types

    // Empty params
    let empty_req = RpcRequest::new(1, "empty".to_string(), vec![]);
    assert_eq!(empty_req.params().len(), 0);

    // Small params
    let small_req = RpcRequest::new(2, "small".to_string(), vec![1, 2, 3]);
    assert_eq!(small_req.params().len(), 3);

    // Large params
    let large_data = vec![0u8; 1000];
    let large_req = RpcRequest::new(3, "large".to_string(), large_data.clone());
    assert_eq!(large_req.params().len(), 1000);

    // Binary params
    let binary_data = vec![0x00, 0xFF, 0x55, 0xAA];
    let binary_req = RpcRequest::new(4, "binary".to_string(), binary_data.clone());
    assert_eq!(binary_req.params(), &binary_data);
}

#[test]
fn test_response_with_different_data() {
    // Test RpcResponse with different data types

    // Empty result
    let empty_resp = RpcResponse::from_result(1, Ok(vec![]));
    assert_eq!(empty_resp.result().unwrap().len(), 0);

    // Small result
    let small_resp = RpcResponse::from_result(2, Ok(vec![1, 2, 3]));
    assert_eq!(small_resp.result().unwrap().len(), 3);

    // Large result
    let large_data = vec![1u8; 1000];
    let large_resp = RpcResponse::from_result(3, Ok(large_data.clone()));
    assert_eq!(large_resp.result().unwrap().len(), 1000);

    // Different error types
    let stream_err_resp =
        RpcResponse::from_result(4, Err(RpcError::StreamError("stream error".to_string())));
    assert!(stream_err_resp.error().is_some());

    let ser_err_resp = RpcResponse::from_result(
        5,
        Err(RpcError::SerializationError(bincode::Error::new(
            bincode::ErrorKind::InvalidBoolEncoding(101),
        ))),
    );
    assert!(ser_err_resp.error().is_some());
}
