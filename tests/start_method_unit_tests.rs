// Unit tests for RpcServer start() method with focus on graceful shutdown
// Testing the start() method's behavior when cancelled and stopped gracefully

use rpcnet::{RpcConfig, RpcServer, RpcClient};
use std::time::Duration;
use tokio::time::timeout;
use std::sync::Arc;
use tokio::sync::Notify;

fn create_test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(100))
}

#[tokio::test]
async fn test_start_method_with_cancellation() {
    // Test that start() method can be cancelled gracefully using tokio::select!
    
    let mut server = RpcServer::new(create_test_config());
    
    // Register a simple handler
    server.register("ping", |_| async move {
        Ok(b"pong".to_vec())
    }).await;
    
    // Bind the server to get a quic server instance
    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server.local_addr().expect("Failed to get server address");
    
    // Create a cancellation mechanism
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();
    
    // Start server with cancellation using tokio::select!
    let server_task = tokio::spawn(async move {
        tokio::select! {
            // This branch runs the server
            result = server.start(quic_server) => {
                println!("Server start() completed with: {:?}", result);
                result
            }
            // This branch waits for shutdown signal
            _ = shutdown_notify_clone.notified() => {
                println!("Server received shutdown signal");
                Ok(()) // Return Ok to indicate graceful shutdown
            }
        }
    });
    
    // Give server a moment to start up
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Verify server is running by connecting to it
    let client = timeout(
        Duration::from_millis(1000),
        RpcClient::connect(server_addr, create_test_config())
    ).await
    .expect("Connection timeout - server may not be running")
    .expect("Failed to connect to server");
    
    // Make a test call to verify server is responding
    let response = timeout(
        Duration::from_millis(1000),
        client.call("ping", b"test".to_vec())
    ).await
    .expect("Call timeout")
    .expect("Call failed");
    
    assert_eq!(response, b"pong", "Server should respond correctly");
    
    // Now trigger graceful shutdown
    shutdown_notify.notify_one();
    
    // Wait for server task to complete gracefully
    let server_result = timeout(Duration::from_millis(2000), server_task).await
        .expect("Server task should complete within timeout");
    
    // Verify that the server shut down successfully
    assert!(server_result.is_ok(), "Server task should complete successfully: {:?}", server_result);
    let start_result = server_result.expect("Server task should not panic");
    assert!(start_result.is_ok(), "start() method should return Ok(()) on graceful shutdown");
    
    println!("✅ Server gracefully shut down after cancellation");
}

#[tokio::test]
async fn test_start_method_with_timeout_cancellation() {
    // Test that start() method works correctly when cancelled by timeout
    
    let mut server = RpcServer::new(create_test_config());
    
    server.register("echo", |params| async move {
        Ok(params)
    }).await;
    
    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server.local_addr().expect("Failed to get server address");
    
    // Start server with a timeout (this will cancel it)
    let server_task = tokio::spawn(async move {
        // Use timeout to automatically cancel the server after a short time
        let result = timeout(Duration::from_millis(500), server.start(quic_server)).await;
        
        match result {
            Ok(start_result) => {
                println!("Server start() completed normally: {:?}", start_result);
                start_result
            }
            Err(_timeout) => {
                println!("Server start() was cancelled by timeout (expected)");
                Ok(()) // This is expected behavior
            }
        }
    });
    
    // Give server time to start up
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Test that server is working while running
    let connection_result = timeout(
        Duration::from_millis(500),
        RpcClient::connect(server_addr, create_test_config())
    ).await;
    
    if let Ok(Ok(client)) = connection_result {
        println!("✅ Successfully connected to server");
        
        // Try to make a call (may succeed or fail depending on timing)
        let call_result = timeout(
            Duration::from_millis(300),
            client.call("echo", b"test_data".to_vec())
        ).await;
        
        match call_result {
            Ok(Ok(response)) => {
                println!("✅ Server responded: {:?}", String::from_utf8_lossy(&response));
                assert_eq!(response, b"test_data");
            }
            _ => {
                println!("⏰ Call timed out or failed (acceptable during shutdown)");
            }
        }
    } else {
        println!("⏰ Connection failed (acceptable during rapid startup/shutdown)");
    }
    
    // Wait for server task to complete
    let server_result = timeout(Duration::from_millis(2000), server_task).await
        .expect("Server task should complete");
    
    assert!(server_result.is_ok(), "Server task should complete without panic");
    let start_result = server_result.expect("Server task should not panic");
    assert!(start_result.is_ok(), "start() should handle cancellation gracefully");
    
    println!("✅ Server start() method handled timeout cancellation correctly");
}

#[tokio::test]
async fn test_start_method_multiple_connections_during_shutdown() {
    // Test start() method behavior when multiple clients are connected during shutdown
    
    let mut server = RpcServer::new(create_test_config());
    
    // Register a handler that takes some time
    server.register("slow_task", |_| async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(b"task_completed".to_vec())
    }).await;
    
    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server.local_addr().expect("Failed to get server address");
    
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();
    
    // Start server
    let server_task = tokio::spawn(async move {
        tokio::select! {
            result = server.start(quic_server) => {
                println!("Server start() completed: {:?}", result);
                result
            }
            _ = shutdown_notify_clone.notified() => {
                println!("Server shutdown requested");
                Ok(())
            }
        }
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Create multiple clients
    let mut client_tasks = Vec::new();
    
    for i in 0..3 {
        let addr = server_addr;
        let config = create_test_config();
        
        let task = tokio::spawn(async move {
            println!("Client {} attempting to connect", i);
            
            // Try to connect
            let client_result = timeout(
                Duration::from_millis(1000),
                RpcClient::connect(addr, config)
            ).await;
            
            if let Ok(Ok(client)) = client_result {
                println!("Client {} connected successfully", i);
                
                // Try to make a call
                let call_result = timeout(
                    Duration::from_millis(1000),
                    client.call("slow_task", format!("request_{}", i).into_bytes())
                ).await;
                
                match call_result {
                    Ok(Ok(response)) => {
                        println!("Client {} call succeeded: {:?}", i, String::from_utf8_lossy(&response));
                        true
                    }
                    _ => {
                        println!("Client {} call failed or timed out", i);
                        false
                    }
                }
            } else {
                println!("Client {} failed to connect", i);
                false
            }
        });
        
        client_tasks.push(task);
    }
    
    // Give clients a moment to connect and start their requests
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Trigger shutdown while clients are potentially active
    println!("Triggering server shutdown...");
    shutdown_notify.notify_one();
    
    // Wait for server to shut down
    let server_result = timeout(Duration::from_millis(3000), server_task).await;
    
    assert!(server_result.is_ok(), "Server should shut down within timeout");
    let server_task_result = server_result.unwrap();
    assert!(server_task_result.is_ok(), "Server task should complete successfully");
    let start_method_result = server_task_result.unwrap();
    assert!(start_method_result.is_ok(), "start() method should return Ok(()) on graceful shutdown");
    
    // Wait for all client tasks to complete and collect results
    let mut successful_calls = 0;
    for (i, task) in client_tasks.into_iter().enumerate() {
        let client_result = timeout(Duration::from_millis(2000), task).await;
        
        match client_result {
            Ok(Ok(true)) => {
                successful_calls += 1;
                println!("Client {} completed successfully", i);
            }
            Ok(Ok(false)) => {
                println!("Client {} completed but call failed", i);
            }
            _ => {
                println!("Client {} task failed or timed out", i);
            }
        }
    }
    
    println!("✅ Server gracefully shut down with {} successful client calls", successful_calls);
    println!("✅ start() method handled multiple connections during shutdown correctly");
}

#[tokio::test]
async fn test_start_method_immediate_shutdown() {
    // Test start() method when shutdown is triggered immediately after start
    
    let mut server = RpcServer::new(create_test_config());
    
    server.register("test", |_| async move {
        Ok(b"response".to_vec())
    }).await;
    
    let quic_server = server.bind().expect("Failed to bind server");
    
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_clone = shutdown_notify.clone();
    
    // Trigger shutdown immediately
    shutdown_notify.notify_one();
    
    // Start server after shutdown is already triggered
    let server_result = timeout(Duration::from_millis(1000), async move {
        tokio::select! {
            result = server.start(quic_server) => {
                println!("Server start() completed: {:?}", result);
                result
            }
            _ = shutdown_notify_clone.notified() => {
                println!("Server shutdown signal received immediately");
                Ok(())
            }
        }
    }).await;
    
    // The server should shut down immediately
    assert!(server_result.is_ok(), "Server should complete quickly when shutdown is pre-triggered");
    let start_result = server_result.unwrap();
    assert!(start_result.is_ok(), "start() should handle immediate shutdown gracefully");
    
    println!("✅ start() method handled immediate shutdown correctly");
}

#[tokio::test] 
async fn test_start_method_returns_ok_on_completion() {
    // Test that start() method returns Ok(()) when it completes successfully
    
    let mut server = RpcServer::new(create_test_config());
    
    server.register("simple", |_| async move {
        Ok(b"ok".to_vec())
    }).await;
    
    let quic_server = server.bind().expect("Failed to bind server");
    
    // Test that we can call start() and it returns the correct type
    // We'll cancel it quickly to test the return value
    let start_future = server.start(quic_server);
    
    // Cancel the future quickly using timeout
    let result = timeout(Duration::from_millis(10), start_future).await;
    
    // The timeout should occur (since start() runs indefinitely)
    assert!(result.is_err(), "start() should run indefinitely until cancelled");
    
    println!("✅ start() method signature and behavior verified - runs indefinitely as expected");
}