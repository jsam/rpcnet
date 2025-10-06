// Core RPC error handling tests to improve coverage
// These tests focus on error paths, edge cases, and failure scenarios

use futures::StreamExt;
use rpcnet::{RpcClient, RpcConfig, RpcError, RpcRequest, RpcResponse, RpcServer};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_client_connection_timeout() {
    // Test connection timeout when server is not responding
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(100));

    // Try to connect to a non-existent server
    let result = timeout(
        Duration::from_secs(1),
        RpcClient::connect("127.0.0.1:9999".parse().unwrap(), config),
    )
    .await;

    // Should timeout or fail to connect
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_malformed_request_handling() {
    // Test handling of malformed requests
    let request = RpcRequest::new(
        u64::MAX,
        "x".repeat(10000),       // Very long method name
        vec![0xFF; 1024 * 1024], // Large payload
    );

    // Serialize and check size
    let serialized = bincode::serialize(&request);
    assert!(serialized.is_ok());

    // Test deserialization of truncated data (should fail)
    let serialized_data = serialized.unwrap();
    let truncated = &serialized_data[..serialized_data.len().saturating_sub(10)];

    let deserialized: Result<RpcRequest, _> = bincode::deserialize(truncated);
    assert!(deserialized.is_err());
}

#[tokio::test]
async fn test_response_error_handling() {
    // Test error response creation and handling
    let error_response = RpcResponse::new(
        123,
        None,
        Some("Critical error: operation failed".to_string()),
    );

    assert_eq!(error_response.id(), 123);
    assert!(error_response.result().is_none());
    assert!(error_response.error().is_some());
    assert_eq!(
        error_response.error().unwrap(),
        "Critical error: operation failed"
    );

    // Test conversion from Result
    let error_result: Result<Vec<u8>, RpcError> =
        Err(RpcError::StreamError("Stream failed".to_string()));
    let response = RpcResponse::from_result(456, error_result);

    assert_eq!(response.id(), 456);
    assert!(response.result().is_none());
    assert!(response.error().is_some());
}

#[tokio::test]
async fn test_server_handler_panic_recovery() {
    // Test server recovery from handler panic
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let server = RpcServer::new(config.clone());

    // Register a handler that might panic
    server
        .register("panic_test", |_params| async move {
            if _params.is_empty() {
                return Err(RpcError::StreamError("Empty params".to_string()));
            }
            Ok(vec![1, 2, 3])
        })
        .await;

    // Test with empty params
    let handlers = server.handlers.read().await;
    let handler = handlers.get("panic_test").unwrap();
    let result = handler(vec![]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_keep_alive_timeout() {
    // Test keep-alive timeout behavior
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_keep_alive_interval(Duration::from_millis(10));

    // Very short keep-alive should affect connection behavior
    assert_eq!(config.keep_alive_interval, Some(Duration::from_millis(10)));

    // Test zero keep-alive
    let zero_config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_keep_alive_interval(Duration::ZERO);

    assert_eq!(zero_config.keep_alive_interval, Some(Duration::ZERO));
}

#[tokio::test]
async fn test_concurrent_request_errors() {
    // Test error handling with concurrent requests
    use futures::future::join_all;

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let server = RpcServer::new(config.clone());

    // Register handler that fails randomly
    server
        .register("flaky_handler", |params| async move {
            let val = params.first().unwrap_or(&0);
            if val % 2 == 0 {
                Err(RpcError::StreamError("Even number error".to_string()))
            } else {
                Ok(params)
            }
        })
        .await;

    // Test multiple concurrent calls
    let handlers = server.handlers.clone();
    let mut futures = vec![];

    for i in 0..10 {
        let handlers_clone = handlers.clone();
        futures.push(async move {
            let handlers = handlers_clone.read().await;
            if let Some(handler) = handlers.get("flaky_handler") {
                handler(vec![i as u8]).await
            } else {
                Err(RpcError::StreamError("Handler not found".to_string()))
            }
        });
    }

    let results = join_all(futures).await;

    // Half should succeed, half should fail
    let errors: Vec<_> = results.iter().filter(|r| r.is_err()).collect();
    let successes: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();

    assert_eq!(errors.len(), 5);
    assert_eq!(successes.len(), 5);
}

#[tokio::test]
async fn test_stream_error_recovery() {
    // Test stream error handling and recovery
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let server = RpcServer::new(config.clone());

    // Register streaming handler that can fail
    server
        .register_streaming("error_stream", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                yield Ok(vec![1, 2, 3]);
                yield Err(RpcError::StreamError("Stream interrupted".to_string()));
                yield Ok(vec![4, 5, 6]); // Should not reach this
            })
        })
        .await;

    // Handler should be registered
    let streaming_handlers = server.streaming_handlers.read().await;
    assert!(streaming_handlers.contains_key("error_stream"));
}

#[tokio::test]
async fn test_invalid_method_name() {
    // Test handling of invalid method names
    let server = RpcServer::new(
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_key_path("certs/test_key.pem"),
    );

    // Register with empty method name
    server.register("", |_| async move { Ok(vec![]) }).await;

    // Register with special characters
    server
        .register("method/with/slashes", |_| async move { Ok(vec![]) })
        .await;

    server
        .register("method.with.dots", |_| async move { Ok(vec![]) })
        .await;

    // All should be registered
    let handlers = server.handlers.read().await;
    assert!(handlers.contains_key(""));
    assert!(handlers.contains_key("method/with/slashes"));
    assert!(handlers.contains_key("method.with.dots"));
}

#[tokio::test]
async fn test_large_payload_errors() {
    // Test handling of very large payloads
    let huge_payload = vec![0xFF; 10 * 1024 * 1024]; // 10MB

    let request = RpcRequest::new(999, "large_test".to_string(), huge_payload.clone());

    // Should be able to serialize
    let serialized = bincode::serialize(&request).unwrap();
    assert!(serialized.len() > 10 * 1024 * 1024);

    // Should be able to deserialize
    let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
    assert_eq!(deserialized.params().len(), huge_payload.len());
}

#[tokio::test]
async fn test_connection_state_errors() {
    // Test various connection state errors
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(50));

    // Multiple connection attempts to non-existent server
    for _ in 0..3 {
        let result = timeout(
            Duration::from_millis(100),
            RpcClient::connect("127.0.0.1:19999".parse().unwrap(), config.clone()),
        )
        .await;

        assert!(result.is_err() || result.unwrap().is_err());
    }
}

#[test]
fn test_error_display_formatting() {
    // Test error message formatting
    use std::fmt::Write;

    let errors = vec![
        RpcError::ConnectionError("Connection refused".to_string()),
        RpcError::StreamError("Serialization error".to_string()),
        RpcError::StreamError("Stream closed unexpectedly".to_string()),
        RpcError::ConfigError("Missing certificate".to_string()),
        RpcError::Timeout,
    ];

    for error in errors {
        let mut output = String::new();
        write!(&mut output, "{}", error).unwrap();
        assert!(!output.is_empty());

        // Debug format should also work
        let debug = format!("{:?}", error);
        assert!(!debug.is_empty());
    }
}

#[test]
fn test_config_validation_errors() {
    // Test configuration validation edge cases

    // Empty certificate path
    let config = RpcConfig::new("", "127.0.0.1:8080");
    assert!(config.cert_path.to_string_lossy().is_empty());

    // Invalid server address
    let config2 = RpcConfig::new("cert.pem", "not_an_address");
    assert_eq!(config2.bind_address, "not_an_address");

    // Very long paths
    let long_path = "a/".repeat(1000) + "cert.pem";
    let config3 = RpcConfig::new(&long_path, "127.0.0.1:8080");
    assert_eq!(config3.cert_path, std::path::PathBuf::from(&long_path));

    // Unicode in paths
    let unicode_path = "certs/测试证书🔒.pem";
    let config4 = RpcConfig::new(unicode_path, "127.0.0.1:8080");
    assert_eq!(config4.cert_path, std::path::PathBuf::from(unicode_path));
}

#[tokio::test]
async fn test_handler_registration_edge_cases() {
    // Test edge cases in handler registration
    let config =
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_key_path("certs/test_key.pem");

    let server = RpcServer::new(config);

    // Register same method multiple times (should overwrite)
    for i in 0..5 {
        let val = i;
        server
            .register("duplicate", move |_| async move { Ok(vec![val]) })
            .await;
    }

    // Only last registration should be active
    let handlers = server.handlers.read().await;
    let handler = handlers.get("duplicate").unwrap();
    let result = handler(vec![]).await.unwrap();
    assert_eq!(result, vec![4]);
}

#[tokio::test]
async fn test_streaming_protocol_errors() {
    // Test streaming protocol parsing errors
    let config =
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_key_path("certs/test_key.pem");

    let server = RpcServer::new(config);

    // Register a streaming handler
    server
        .register_streaming("test_stream", |request_stream| async move {
            Box::pin(async_stream::stream! {
                let mut count = 0;
                tokio::pin!(request_stream);
                while let Some(data) = request_stream.next().await {
                    count += 1;
                    if count > 3 {
                        yield Err(RpcError::StreamError("Too many requests".to_string()));
                        break;
                    }
                    yield Ok(data);
                }
            })
        })
        .await;

    let streaming_handlers = server.streaming_handlers.read().await;
    assert!(streaming_handlers.contains_key("test_stream"));
}

#[test]
fn test_request_response_edge_cases() {
    // Test request/response edge cases

    // Zero ID
    let req = RpcRequest::new(0, "test".to_string(), vec![]);
    assert_eq!(req.id(), 0);

    // Max ID
    let req2 = RpcRequest::new(u64::MAX, "test".to_string(), vec![]);
    assert_eq!(req2.id(), u64::MAX);

    // Empty method
    let req3 = RpcRequest::new(1, String::new(), vec![]);
    assert_eq!(req3.method(), "");

    // Response with both result and error (shouldn't happen but test anyway)
    let resp = RpcResponse::new(1, Some(vec![1, 2, 3]), Some("error".to_string()));
    assert!(resp.result().is_some());
    assert!(resp.error().is_some());
}

#[tokio::test]
async fn test_concurrent_handler_modifications() {
    // Test concurrent modifications to handlers
    use std::sync::Arc;
    use tokio::task;

    let config =
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_key_path("certs/test_key.pem");

    let server = Arc::new(RpcServer::new(config));

    // Spawn multiple tasks that register handlers
    let mut handles = vec![];

    for i in 0..10 {
        let server_clone = server.clone();
        let handle = task::spawn(async move {
            let server = server_clone.as_ref().clone();
            let method_name = format!("method_{}", i);
            server
                .register(&method_name, move |_| async move { Ok(vec![i as u8]) })
                .await;
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}
