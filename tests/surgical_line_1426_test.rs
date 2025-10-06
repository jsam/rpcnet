#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
// Surgical test to hit EXACTLY line 1426 in start() method
#![allow(clippy::collapsible_if)]
#![allow(clippy::get_first)]
#![allow(clippy::useless_vec)]
// Line 1426: let _ = stream.send(response_data.into()).await;
// This line is inside: if let Ok(request) = bincode::deserialize::<RpcRequest>(&request_data)

use rpcnet::{RpcClient, RpcConfig, RpcServer};
use std::time::Duration;
use tokio::time::timeout;

fn create_test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(100))
}

#[tokio::test]
async fn test_surgical_line_1426_bincode_path() {
    // This test is designed to hit the EXACT path:
    // 1. Data comes in via stream.receive()
    // 2. Gets parsed via bincode::deserialize::<RpcRequest>(&request_data)
    // 3. Handler is found and executed
    // 4. Response is created via RpcResponse::from_result()
    // 5. Response gets serialized via bincode::serialize(&response)
    // 6. Line 1426 executes: let _ = stream.send(response_data.into()).await;

    let mut server = RpcServer::new(create_test_config());

    // Register a handler that will DEFINITELY be found and executed
    server
        .register("surgical_test", |params| async move {
            println!("SURGICAL HANDLER EXECUTED with params: {:?}", params);
            // Return a response that will definitely serialize successfully
            Ok(b"surgical_response_success".to_vec())
        })
        .await;

    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server
        .local_addr()
        .expect("Failed to get server address");

    println!("Starting surgical test for line 1426...");

    // Start the server
    let server_handle = tokio::spawn(async move { server.start(quic_server).await });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(300)).await;

    println!("Connecting client to hit the exact bincode deserialization path...");

    // Connect and make a call that will definitely hit the bincode path
    let client = timeout(
        Duration::from_millis(2000),
        RpcClient::connect(server_addr, create_test_config()),
    )
    .await
    .expect("Client connection timeout")
    .expect("Client connection failed");

    println!("Making surgical RPC call...");

    // Make a call that should go through the exact code path
    let response = timeout(
        Duration::from_millis(2000),
        client.call("surgical_test", b"surgical_params".to_vec()),
    )
    .await
    .expect("Call timeout")
    .expect("Call failed");

    assert_eq!(response, b"surgical_response_success");

    println!("✅ SURGICAL TEST SUCCESS!");
    println!("   - Request went through bincode::deserialize::<RpcRequest>");
    println!("   - Handler was found and executed");
    println!("   - Response went through bincode::serialize");
    println!("   - Line 1426 should have been executed: stream.send(response_data.into()).await");

    // Make additional calls to ensure multiple hits
    for i in 0..3 {
        let params = format!("surgical_param_{}", i).into_bytes();
        let response = timeout(
            Duration::from_millis(1000),
            client.call("surgical_test", params),
        )
        .await
        .expect("Additional call timeout")
        .expect("Additional call failed");

        assert_eq!(response, b"surgical_response_success");
        println!("✅ Additional surgical call {} completed", i);
    }

    println!("✅ ALL SURGICAL CALLS COMPLETED - Line 1426 hit multiple times!");

    // Clean shutdown
    drop(client);
    server_handle.abort();
}

#[tokio::test]
async fn test_line_1426_with_unknown_method() {
    // Test the unknown method path to ensure we also hit line 1426 in error cases
    // This should go through the: None => RpcResponse::new(..., "Unknown method") path

    let mut server = RpcServer::new(create_test_config());

    // Register ONE handler, but we'll call a DIFFERENT method
    server
        .register("known_method", |_| async move { Ok(b"known".to_vec()) })
        .await;

    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server
        .local_addr()
        .expect("Failed to get server address");

    let server_handle = tokio::spawn(async move { server.start(quic_server).await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = timeout(
        Duration::from_millis(2000),
        RpcClient::connect(server_addr, create_test_config()),
    )
    .await
    .expect("Client connection timeout")
    .expect("Client connection failed");

    println!("Testing unknown method path for line 1426...");

    // Call an UNKNOWN method - this should trigger the "Unknown method" response
    // and still go through line 1426
    let error_result = client.call("unknown_method", b"test".to_vec()).await;

    // This should fail, but importantly, it should have gone through line 1426
    assert!(error_result.is_err(), "Unknown method should return error");

    println!("✅ Unknown method path tested - should have hit line 1426 for error response");

    // Also test the known method to ensure both paths work
    let success_result = client
        .call("known_method", b"test".to_vec())
        .await
        .expect("Known method should work");
    assert_eq!(success_result, b"known");

    println!("✅ Known method path also tested - line 1426 hit again");

    drop(client);
    server_handle.abort();
}

#[tokio::test]
async fn test_line_1426_with_various_response_sizes() {
    // Test line 1426 with different response sizes to ensure serialization always works

    let mut server = RpcServer::new(create_test_config());

    server
        .register("size_test", |params| async move {
            match params.get(0).copied().unwrap_or(0) {
                0 => Ok(vec![]),            // Empty response
                1 => Ok(b"small".to_vec()), // Small response
                2 => Ok(vec![42u8; 1000]),  // Medium response (1KB)
                3 => Ok(vec![99u8; 10000]), // Large response (10KB)
                _ => Ok(b"default".to_vec()),
            }
        })
        .await;

    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server
        .local_addr()
        .expect("Failed to get server address");

    let server_handle = tokio::spawn(async move { server.start(quic_server).await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = timeout(
        Duration::from_millis(2000),
        RpcClient::connect(server_addr, create_test_config()),
    )
    .await
    .expect("Client connection timeout")
    .expect("Client connection failed");

    println!("Testing line 1426 with various response sizes...");

    // Test different response sizes
    let test_cases = [
        (0, 0, "empty response"),
        (1, 5, "small response"),
        (2, 1000, "medium response (1KB)"),
        (3, 10000, "large response (10KB)"),
    ];

    for (param, expected_size, description) in test_cases {
        let response = timeout(
            Duration::from_millis(3000),
            client.call("size_test", vec![param]),
        )
        .await
        .expect("Size test timeout")
        .expect("Size test failed");

        assert_eq!(
            response.len(),
            expected_size,
            "Size mismatch for {}",
            description
        );
        println!("✅ {} serialized and sent via line 1426", description);
    }

    println!("✅ ALL RESPONSE SIZES TESTED - Line 1426 exercised with various data");

    drop(client);
    server_handle.abort();
}

#[tokio::test]
async fn test_concurrent_calls_hitting_line_1426() {
    // Test concurrent calls to ensure line 1426 is hit under concurrent load

    let mut server = RpcServer::new(create_test_config());

    server
        .register("concurrent_test", |params| async move {
            // Add a small delay to simulate processing
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(format!("processed_{}", params.len()).into_bytes())
        })
        .await;

    let quic_server = server.bind().expect("Failed to bind server");
    let server_addr = quic_server
        .local_addr()
        .expect("Failed to get server address");

    let server_handle = tokio::spawn(async move { server.start(quic_server).await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = timeout(
        Duration::from_millis(2000),
        RpcClient::connect(server_addr, create_test_config()),
    )
    .await
    .expect("Client connection timeout")
    .expect("Client connection failed");

    println!("Testing concurrent calls to hit line 1426 multiple times...");

    // Launch multiple concurrent calls
    let mut tasks = Vec::new();
    for i in 0..10 {
        let client_ref = &client;
        let task = async move {
            let params = format!("concurrent_param_{}", i).into_bytes();
            client_ref.call("concurrent_test", params).await
        };
        tasks.push(task);
    }

    let results = futures::future::join_all(tasks).await;
    let successful_calls = results.iter().filter(|r| r.is_ok()).count();

    println!(
        "✅ Concurrent test completed: {}/10 calls successful",
        successful_calls
    );
    println!("   Each successful call hit line 1426 for response sending");

    assert!(
        successful_calls >= 8,
        "At least 8/10 concurrent calls should succeed"
    );

    drop(client);
    server_handle.abort();

    println!("✅ CONCURRENT LINE 1426 TEST COMPLETED!");
}
