use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(test)]
mod integration_tests {
    use super::*;

    // Helper function to create test configuration
    fn test_config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(30))
    }

    // Helper function to start a test server with given handlers
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

        // Give server time to start
        sleep(Duration::from_millis(10)).await;

        Ok((addr, handle))
    }

    // ==========================
    // Basic Client-Server Communication
    // ==========================
    #[tokio::test]
    async fn test_basic_echo_communication() {
        let server = RpcServer::new(test_config());

        server
            .register("echo", |params| async move { Ok(params) })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        let test_data = b"Hello, World!".to_vec();
        let response = client.call("echo", test_data.clone()).await.unwrap();

        assert_eq!(response, test_data);
    }

    #[tokio::test]
    async fn test_multiple_method_registration() {
        let server = RpcServer::new(test_config());

        server
            .register("add", |params| async move {
                let nums: Vec<i32> = bincode::deserialize(&params).unwrap();
                let result = nums.iter().sum::<i32>();
                Ok(bincode::serialize(&result).unwrap())
            })
            .await;

        server
            .register("multiply", |params| async move {
                let nums: Vec<i32> = bincode::deserialize(&params).unwrap();
                let result = nums.iter().product::<i32>();
                Ok(bincode::serialize(&result).unwrap())
            })
            .await;

        server
            .register("count", |params| async move {
                let count = params.len() as u32;
                Ok(bincode::serialize(&count).unwrap())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test add
        let nums = vec![1, 2, 3, 4, 5];
        let params = bincode::serialize(&nums).unwrap();
        let response = client.call("add", params).await.unwrap();
        let result: i32 = bincode::deserialize(&response).unwrap();
        assert_eq!(result, 15);

        // Test multiply
        let nums = vec![2, 3, 4];
        let params = bincode::serialize(&nums).unwrap();
        let response = client.call("multiply", params).await.unwrap();
        let result: i32 = bincode::deserialize(&response).unwrap();
        assert_eq!(result, 24);

        // Test count
        let data = vec![1u8; 100];
        let response = client.call("count", data).await.unwrap();
        let result: u32 = bincode::deserialize(&response).unwrap();
        assert_eq!(result, 100);
    }

    #[tokio::test]
    async fn test_empty_params_and_response() {
        let server = RpcServer::new(test_config());

        server
            .register("ping", |_params| async move { Ok(b"pong".to_vec()) })
            .await;

        server
            .register("empty", |_params| async move { Ok(vec![]) })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test ping with empty params
        let response = client.call("ping", vec![]).await.unwrap();
        assert_eq!(response, b"pong");

        // Test empty response
        let response = client.call("empty", vec![]).await.unwrap();
        assert_eq!(response, vec![]);
    }

    // ==========================
    // Error Scenarios
    // ==========================
    #[tokio::test]
    async fn test_unknown_method_error() {
        let server = RpcServer::new(test_config());
        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        let result = client.call("nonexistent_method", vec![]).await;

        match result {
            Err(RpcError::StreamError(msg)) => {
                assert!(msg.contains("Unknown method"));
            }
            _ => panic!("Expected StreamError for unknown method"),
        }
    }

    #[tokio::test]
    async fn test_handler_error_propagation() {
        let server = RpcServer::new(test_config());

        server
            .register("error_handler", |_params| async move {
                Err(RpcError::StreamError("Handler error".to_string()))
            })
            .await;

        server
            .register("panic_handler", |_params| async move {
                panic!("Handler panic");
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test explicit error return
        let result = client.call("error_handler", vec![]).await;
        match result {
            Err(RpcError::StreamError(msg)) => {
                assert_eq!(msg, "Stream error: Handler error");
            }
            _ => panic!("Expected StreamError from handler"),
        }
    }

    #[tokio::test]
    async fn test_serialization_errors() {
        let server = RpcServer::new(test_config());

        server
            .register("deserialize_test", |params| async move {
                // Try to deserialize as a specific type that will fail
                let _: Result<String, _> = bincode::deserialize(&params);
                Ok(b"success".to_vec())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Send invalid binary data
        let invalid_data = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let response = client.call("deserialize_test", invalid_data).await.unwrap();
        assert_eq!(response, b"success");
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        // Try to connect to a non-existent server
        let invalid_addr = "127.0.0.1:1".parse().unwrap();
        let result = RpcClient::connect(invalid_addr, test_config()).await;

        match result {
            Err(RpcError::ConnectionError(_)) => {
                // Expected
            }
            _ => panic!("Expected ConnectionError for invalid address"),
        }
    }

    #[tokio::test]
    async fn test_request_timeout() {
        let server = RpcServer::new(test_config());

        server
            .register("slow_handler", |_params| async move {
                // Sleep longer than the default timeout (both 2s test and 30s coverage mode)
                sleep(Duration::from_secs(35)).await;
                Ok(b"too_late".to_vec())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        let result = client.call("slow_handler", vec![]).await;

        match result {
            Err(RpcError::Timeout) => {
                // Expected
            }
            _ => panic!("Expected Timeout error for slow handler"),
        }
    }

    // ==========================
    // Concurrent Operations
    // ==========================
    #[tokio::test]
    async fn test_concurrent_requests_single_client() {
        let request_counter = Arc::new(AtomicU64::new(0));
        let counter_clone = request_counter.clone();

        let server = RpcServer::new(test_config());

        server
            .register("increment", move |_params| {
                let counter = counter_clone.clone();
                async move {
                    let value = counter.fetch_add(1, Ordering::SeqCst);
                    Ok(bincode::serialize(&value).unwrap())
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = Arc::new(RpcClient::connect(addr, test_config()).await.unwrap());

        // Launch 10 concurrent requests
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let client_clone = client.clone();
            let task = tokio::spawn(async move {
                let response = client_clone.call("increment", vec![]).await.unwrap();
                bincode::deserialize::<u64>(&response).unwrap()
            });
            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await.unwrap());
        }

        // All requests should succeed
        assert_eq!(results.len(), 10);

        // The counter should have been incremented 10 times
        assert_eq!(request_counter.load(Ordering::SeqCst), 10);

        // Results should be unique (no race conditions)
        results.sort();
        for (i, &value) in results.iter().enumerate() {
            assert_eq!(value, i as u64);
        }
    }

    #[tokio::test]
    async fn test_multiple_clients_concurrent_access() {
        let request_counter = Arc::new(AtomicU64::new(0));
        let counter_clone = request_counter.clone();

        let server = RpcServer::new(test_config());

        server
            .register("global_increment", move |_params| {
                let counter = counter_clone.clone();
                async move {
                    let value = counter.fetch_add(1, Ordering::SeqCst);
                    // Add small delay to increase chance of race conditions if they exist
                    sleep(Duration::from_millis(1)).await;
                    Ok(bincode::serialize(&value).unwrap())
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();

        // Create multiple clients
        let mut client_tasks = Vec::new();
        for client_id in 0..5 {
            let test_config = test_config();
            let task = tokio::spawn(async move {
                let client = RpcClient::connect(addr, test_config).await.unwrap();

                // Each client makes multiple requests
                let client = Arc::new(client);
                let mut requests = Vec::new();
                for _ in 0..4 {
                    let client_clone = client.clone();
                    let request = tokio::spawn(async move {
                        let response = client_clone.call("global_increment", vec![]).await.unwrap();
                        bincode::deserialize::<u64>(&response).unwrap()
                    });
                    requests.push(request);
                }

                let mut results = Vec::new();
                for request in requests {
                    results.push(request.await.unwrap());
                }
                (client_id, results)
            });
            client_tasks.push(task);
        }

        let mut all_results = Vec::new();
        for task in client_tasks {
            let (client_id, results) = task.await.unwrap();
            let results_len = results.len();
            all_results.extend(results);
            println!("Client {} completed {} requests", client_id, results_len);
        }

        // Should have 5 clients × 4 requests = 20 total requests
        assert_eq!(all_results.len(), 20);
        assert_eq!(request_counter.load(Ordering::SeqCst), 20);

        // All results should be unique (proper concurrent handling)
        all_results.sort();
        for (i, &value) in all_results.iter().enumerate() {
            assert_eq!(value, i as u64);
        }
    }

    // ==========================
    // Large Payload Tests
    // ==========================
    #[tokio::test]
    async fn test_large_request_payload() {
        let server = RpcServer::new(test_config());

        server
            .register("size_check", |params| async move {
                let size = params.len() as u32;
                Ok(bincode::serialize(&size).unwrap())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test various payload sizes
        let sizes = vec![
            1024,      // 1KB
            10_240,    // 10KB
            102_400,   // 100KB
            1_024_000, // 1MB
        ];

        for size in sizes {
            let large_payload = vec![0xAA; size];
            let response = client.call("size_check", large_payload).await.unwrap();
            let returned_size: u32 = bincode::deserialize(&response).unwrap();
            assert_eq!(returned_size, size as u32);
        }
    }

    #[tokio::test]
    async fn test_large_response_payload() {
        let server = RpcServer::new(test_config());

        server
            .register("generate_data", |params| async move {
                let size: u32 = bincode::deserialize(&params).unwrap();
                let data = vec![0xFF; size as usize];
                Ok(data)
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test various response sizes
        let sizes = vec![1024u32, 10_240, 102_400, 512_000]; // Up to 512KB

        for size in sizes {
            let params = bincode::serialize(&size).unwrap();
            let response = client.call("generate_data", params).await.unwrap();
            assert_eq!(response.len(), size as usize);
            assert!(response.iter().all(|&b| b == 0xFF));
        }
    }

    // ==========================
    // Stress and Performance Tests
    // ==========================
    #[tokio::test]
    async fn test_rapid_sequential_requests() {
        let server = RpcServer::new(test_config());

        server
            .register("counter", |params| async move {
                let input: u32 = bincode::deserialize(&params).unwrap();
                Ok(bincode::serialize(&(input + 1)).unwrap())
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        let start = Instant::now();
        let num_requests = 100;

        for i in 0..num_requests {
            let params = bincode::serialize(&i).unwrap();
            let response = client.call("counter", params).await.unwrap();
            let result: u32 = bincode::deserialize(&response).unwrap();
            assert_eq!(result, i + 1);
        }

        let elapsed = start.elapsed();
        let requests_per_second = num_requests as f64 / elapsed.as_secs_f64();

        println!("Sequential requests: {} req/sec", requests_per_second);

        // Should be able to handle at least 10 requests per second
        assert!(requests_per_second > 10.0);
    }

    #[tokio::test]
    async fn test_connection_reuse() {
        let server = RpcServer::new(test_config());

        server
            .register("ping", |_params| async move { Ok(b"pong".to_vec()) })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Make multiple requests on the same connection
        for i in 0..20 {
            let response = client.call("ping", vec![]).await.unwrap();
            assert_eq!(response, b"pong", "Request {} failed", i);
        }
    }

    // ==========================
    // Server State and Lifecycle
    // ==========================
    #[tokio::test]
    async fn test_server_handler_state() {
        use std::sync::Mutex;
        let state = Arc::new(Mutex::new(Vec::<String>::new()));
        let state_clone = state.clone();

        let server = RpcServer::new(test_config());

        server
            .register("add_item", move |params| {
                let state = state_clone.clone();
                async move {
                    let item: String = bincode::deserialize(&params).unwrap();
                    state.lock().unwrap().push(item);
                    let count = state.lock().unwrap().len();
                    Ok(bincode::serialize(&count).unwrap())
                }
            })
            .await;

        server
            .register("get_items", move |_params| {
                let state = state.clone();
                async move {
                    let items = state.lock().unwrap().clone();
                    Ok(bincode::serialize(&items).unwrap())
                }
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Add some items
        let items = ["item1", "item2", "item3"];
        for (i, item) in items.iter().enumerate() {
            let params = bincode::serialize(&item.to_string()).unwrap();
            let response = client.call("add_item", params).await.unwrap();
            let count: usize = bincode::deserialize(&response).unwrap();
            assert_eq!(count, i + 1);
        }

        // Get all items
        let response = client.call("get_items", vec![]).await.unwrap();
        let retrieved_items: Vec<String> = bincode::deserialize(&response).unwrap();
        assert_eq!(retrieved_items.len(), 3);
        assert_eq!(retrieved_items, vec!["item1", "item2", "item3"]);
    }

    // ==========================
    // Binary Data Handling
    // ==========================
    #[tokio::test]
    async fn test_binary_data_integrity() {
        let server = RpcServer::new(test_config());

        server
            .register("binary_echo", |params| async move { Ok(params) })
            .await;

        server
            .register("binary_transform", |params| async move {
                // XOR each byte with 0xFF
                let transformed: Vec<u8> = params.iter().map(|&b| b ^ 0xFF).collect();
                Ok(transformed)
            })
            .await;

        let (addr, _handle) = start_test_server(server).await.unwrap();
        let client = RpcClient::connect(addr, test_config()).await.unwrap();

        // Test with various binary patterns
        let test_patterns = vec![
            vec![0x00, 0xFF, 0xAA, 0x55],
            vec![0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80],
            (0..256).map(|i| i as u8).collect::<Vec<u8>>(), // All byte values
        ];

        for pattern in test_patterns {
            // Test echo
            let response = client.call("binary_echo", pattern.clone()).await.unwrap();
            assert_eq!(response, pattern);

            // Test transform
            let response = client
                .call("binary_transform", pattern.clone())
                .await
                .unwrap();
            let expected: Vec<u8> = pattern.iter().map(|&b| b ^ 0xFF).collect();
            assert_eq!(response, expected);
        }
    }

    // ==========================
    // Configuration and Network Tests
    // ==========================
    #[tokio::test]
    async fn test_different_bind_addresses() {
        // Test IPv4 localhost
        let config_v4 = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost");

        let mut server = RpcServer::new(config_v4.clone());
        server
            .register("test", |_| async move { Ok(b"ok".to_vec()) })
            .await;

        let quic_server = server.bind().unwrap();
        let addr = quic_server.local_addr().unwrap();

        // Verify it's an IPv4 address
        assert!(addr.is_ipv4());
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn test_keep_alive_configuration() {
        let config_short = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(5));

        let config_long = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(300));

        // Both configurations should be valid
        assert_eq!(
            config_short.keep_alive_interval,
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            config_long.keep_alive_interval,
            Some(Duration::from_secs(300))
        );
    }
}
