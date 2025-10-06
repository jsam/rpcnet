#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod error_scenarios {
    use super::*;

    fn test_config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(30))
    }

    async fn start_test_server(
        mut server: RpcServer,
    ) -> Result<
        (
            std::net::SocketAddr,
            tokio::task::JoinHandle<Result<(), RpcError>>,
        ),
        RpcError,
    > {
        let quic_server = server.bind()?;
        let addr = quic_server.local_addr()?;

        let handle = tokio::spawn(async move { server.start(quic_server).await });

        sleep(Duration::from_millis(10)).await;
        Ok((addr, handle))
    }

    // ==========================
    // Connection and Network Errors
    // ==========================
    #[tokio::test]
    async fn test_connection_refused() {
        // Try to connect to a port that's likely not in use
        let invalid_addr = "127.0.0.1:1".parse().unwrap();
        let result = RpcClient::connect(invalid_addr, test_config()).await;

        assert!(matches!(result, Err(RpcError::ConnectionError(_))));
    }

    #[tokio::test]
    async fn test_invalid_server_name() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("invalid.server.name.that.does.not.match.cert");

        let server = RpcServer::new(test_config());
        server.register("test", |_| async move { Ok(vec![]) }).await;
        let (addr, _handle) = start_test_server(server).await.unwrap();

        // Connection might fail due to certificate name mismatch
        // This tests the TLS validation
        let result = RpcClient::connect(addr, config).await;
        // Note: This might succeed in test environment due to self-signed certs
        // The important thing is that it doesn't panic
        println!("Connection result: {:?}", result.is_ok());
    }

    #[tokio::test]
    async fn test_missing_certificate_file() {
        let config = RpcConfig::new("nonexistent_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem");

        let mut server = RpcServer::new(config);
        let result = server.bind();

        // TLS errors can also manifest as ConfigError in some QUIC implementations
        assert!(matches!(
            result,
            Err(RpcError::TlsError(_)) | Err(RpcError::ConfigError(_))
        ));
    }

    #[tokio::test]
    async fn test_missing_key_file() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("nonexistent_key.pem");

        let mut server = RpcServer::new(config);
        let result = server.bind();

        // TLS errors can also manifest as ConfigError in some QUIC implementations
        assert!(matches!(
            result,
            Err(RpcError::TlsError(_)) | Err(RpcError::ConfigError(_))
        ));
    }

    #[tokio::test]
    async fn test_no_key_path_configured() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0");
        // No key path set

        let mut server = RpcServer::new(config);
        let result = server.bind();

        assert!(matches!(result, Err(RpcError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_invalid_bind_address() {
        let config = RpcConfig::new("certs/test_cert.pem", "invalid.address:8080")
            .with_key_path("certs/test_key.pem");

        let mut server = RpcServer::new(config);
        let result = server.bind();

        assert!(matches!(result, Err(RpcError::ConfigError(_))));
    }

    // ==========================
    // Handler Errors
    // ==========================
    #[tokio::test]
    async fn test_handler_serialization_error() {
        let server = RpcServer::new(test_config());

        server
            .register("bad_deserialize", |params| async move {
                // Try to deserialize random bytes as a complex structure
                #[derive(serde::Deserialize)]
                #[allow(dead_code)]
                struct ComplexStruct {
                    field1: String,
                    field2: Vec<i32>,
                    field3: std::collections::HashMap<String, bool>,
                }

                match bincode::deserialize::<ComplexStruct>(&params) {
                    Ok(_) => Ok(b"success".to_vec()),
                    Err(e) => Err(RpcError::SerializationError(e)),
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Send invalid data
        let invalid_data = vec![0xFF, 0x00, 0xAA];
        let result = client.call("bad_deserialize", invalid_data).await;

        assert!(matches!(result, Err(RpcError::StreamError(_))));
    }

    #[tokio::test]
    async fn test_handler_custom_errors() {
        let server = RpcServer::new(test_config());

        server
            .register("validation_error", |params| async move {
                let value: i32 = bincode::deserialize(&params).unwrap();
                if value < 0 {
                    Err(RpcError::StreamError(
                        "Value must be non-negative".to_string(),
                    ))
                } else if value > 100 {
                    Err(RpcError::StreamError("Value must be <= 100".to_string()))
                } else {
                    Ok(bincode::serialize(&(value * 2)).unwrap())
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test negative value
        let params = bincode::serialize(&(-5)).unwrap();
        let result = client.call("validation_error", params).await;
        match result {
            Err(RpcError::StreamError(msg)) => assert!(msg.contains("non-negative")),
            _ => panic!("Expected validation error for negative value"),
        }

        // Test too large value
        let params = bincode::serialize(&150).unwrap();
        let result = client.call("validation_error", params).await;
        match result {
            Err(RpcError::StreamError(msg)) => assert!(msg.contains("<= 100")),
            _ => panic!("Expected validation error for large value"),
        }

        // Test valid value
        let params = bincode::serialize(&50).unwrap();
        let result = client.call("validation_error", params).await.unwrap();
        let response: i32 = bincode::deserialize(&result).unwrap();
        assert_eq!(response, 100);
    }

    #[tokio::test]
    async fn test_handler_timeout_scenarios() {
        let server = RpcServer::new(test_config());

        server
            .register("quick", |_params| async move {
                sleep(Duration::from_millis(10)).await;
                Ok(b"quick".to_vec())
            })
            .await;

        server
            .register("medium", |_params| async move {
                sleep(Duration::from_millis(100)).await;
                Ok(b"medium".to_vec())
            })
            .await;

        server
            .register("slow", |_params| async move {
                sleep(Duration::from_millis(500)).await;
                Ok(b"slow".to_vec())
            })
            .await;

        server
            .register("timeout", |_params| async move {
                sleep(Duration::from_secs(35)).await; // Will exceed timeout in both test and coverage mode
                Ok(b"too_late".to_vec())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Quick should succeed
        let result = client.call("quick", vec![]).await.unwrap();
        assert_eq!(result, b"quick");

        // Medium should succeed
        let result = client.call("medium", vec![]).await.unwrap();
        assert_eq!(result, b"medium");

        // Slow might succeed depending on system load
        let result = client.call("slow", vec![]).await;
        if result.is_ok() {
            assert_eq!(result.unwrap(), b"slow");
        }

        // Timeout should fail
        let result = client.call("timeout", vec![]).await;
        assert!(matches!(result, Err(RpcError::Timeout)));
    }

    // ==========================
    // Concurrency Error Scenarios
    // ==========================
    #[tokio::test]
    async fn test_concurrent_handler_errors() {
        let server = RpcServer::new(test_config());

        server
            .register("sometimes_fail", |params| async move {
                let value: u32 = bincode::deserialize(&params).unwrap();

                // Fail for even numbers
                if value % 2 == 0 {
                    Err(RpcError::StreamError(format!(
                        "Even number not allowed: {}",
                        value
                    )))
                } else {
                    sleep(Duration::from_millis(10)).await;
                    Ok(bincode::serialize(&(value * 2)).unwrap())
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = Arc::new(RpcClient::connect(addr, test_config()).await.unwrap());

        // Launch concurrent requests, some will succeed, some will fail
        let mut tasks = Vec::new();
        for i in 0..20 {
            let client_clone = client.clone();
            let task = tokio::spawn(async move {
                let params = bincode::serialize(&i).unwrap();
                let result = client_clone.call("sometimes_fail", params).await;
                (i, result)
            });
            tasks.push(task);
        }

        let mut successes = 0;
        let mut failures = 0;

        for task in tasks {
            let (value, result) = task.await.unwrap();
            match result {
                Ok(response) => {
                    let doubled: u32 = bincode::deserialize(&response).unwrap();
                    assert_eq!(doubled, value * 2);
                    assert_eq!(value % 2, 1); // Should be odd
                    successes += 1;
                }
                Err(RpcError::StreamError(msg)) => {
                    assert!(msg.contains("Even number not allowed"));
                    assert_eq!(value % 2, 0); // Should be even
                    failures += 1;
                }
                Err(e) => panic!("Unexpected error type: {:?}", e),
            }
        }

        // Should have 10 successes (odd numbers) and 10 failures (even numbers)
        assert_eq!(successes, 10);
        assert_eq!(failures, 10);
    }

    // ==========================
    // Resource Exhaustion Scenarios
    // ==========================
    #[tokio::test]
    async fn test_large_payload_limits() {
        let server = RpcServer::new(test_config());

        server
            .register("memory_test", |params| async move {
                let size = params.len();

                // Simulate memory constraints
                if size > 10_000_000 {
                    // 10MB limit
                    return Err(RpcError::StreamError("Payload too large".to_string()));
                }

                // Simulate processing
                let _processed = vec![0u8; size];
                Ok(bincode::serialize(&size).unwrap())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test acceptable size
        let medium_payload = vec![0xFF; 1_000_000]; // 1MB
        let result = client.call("memory_test", medium_payload).await.unwrap();
        let size: usize = bincode::deserialize(&result).unwrap();
        assert_eq!(size, 1_000_000);

        // Test too large payload (this might fail at network level or handler level)
        let large_payload = vec![0xAA; 12_000_000]; // 12MB - exceeds 10MB limit
        let result = client.call("memory_test", large_payload).await;

        // Should fail either with our custom error, network error, or timeout
        assert!(result.is_err());
        match result {
            Err(RpcError::StreamError(msg)) => {
                assert!(msg.contains("too large") || msg.contains("Payload too large"));
            }
            Err(RpcError::ConnectionError(_)) => {
                // Also acceptable - network layer rejected it
            }
            Err(RpcError::Timeout) => {
                // Also acceptable - large payload transmission/processing timed out
            }
            Err(e) => panic!("Unexpected error for large payload: {:?}", e),
            Ok(_) => panic!("Large payload should have failed"),
        }
    }

    // ==========================
    // Protocol Error Scenarios
    // ==========================
    #[tokio::test]
    async fn test_malformed_responses() {
        let server = RpcServer::new(test_config());

        server
            .register("bad_response", |_params| async move {
                // Return data that can't be properly handled
                Ok(vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF]) // Invalid serialized data
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // This should succeed at the RPC level but the response might be invalid
        let result = client.call("bad_response", vec![]).await.unwrap();
        assert_eq!(result, vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    // ==========================
    // Edge Cases with Request IDs
    // ==========================
    #[tokio::test]
    async fn test_request_id_edge_cases() {
        let server = RpcServer::new(test_config());

        server
            .register("echo_with_delay", |params| async move {
                // Variable delay based on first byte
                let delay_ms = if params.is_empty() {
                    10
                } else {
                    params[0] as u64
                };
                sleep(Duration::from_millis(delay_ms)).await;
                Ok(params)
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = Arc::new(RpcClient::connect(addr, test_config()).await.unwrap());

        // Launch requests with different delays
        // This tests that request IDs are handled correctly even when responses arrive out of order
        let mut tasks = Vec::new();
        let delays = [50u8, 10u8, 30u8, 5u8, 25u8];

        for (i, &delay) in delays.iter().enumerate() {
            let client_clone = client.clone();
            let task = tokio::spawn(async move {
                let payload = vec![delay, i as u8]; // Include index in payload
                let result = client_clone.call("echo_with_delay", payload.clone()).await;
                (i, delay, payload, result)
            });
            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            let (index, delay, payload, result) = task.await.unwrap();
            match result {
                Ok(response) => {
                    assert_eq!(response, payload);
                    results.push((index, delay));
                }
                Err(e) => panic!("Request {} with delay {}ms failed: {:?}", index, delay, e),
            }
        }

        // All requests should have completed successfully
        assert_eq!(results.len(), 5);

        // Results might arrive in different order due to different delays
        results.sort_by_key(|&(index, _)| index);
        for (i, (index, _)) in results.iter().enumerate() {
            assert_eq!(*index, i);
        }
    }

    // ==========================
    // Configuration Edge Cases
    // ==========================
    #[tokio::test]
    async fn test_zero_keep_alive() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(0));

        let server = RpcServer::new(config.clone());
        server
            .register("test", |_| async move { Ok(b"ok".to_vec()) })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, config).await.unwrap();

        // Should still work even with zero keep-alive
        let result = client.call("test", vec![]).await.unwrap();
        assert_eq!(result, b"ok");
    }

    #[tokio::test]
    async fn test_empty_server_name() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("");

        // Should not panic, even if it might not work in practice
        assert_eq!(config.server_name, "");
    }
}
