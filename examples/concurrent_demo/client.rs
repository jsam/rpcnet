//! Concurrent demo client using generated code.

#[path = "generated/concurrentdemo/mod.rs"]
mod concurrentdemo;

use concurrentdemo::client::ConcurrentDemoClient;
use concurrentdemo::{AsyncTaskRequest, ComputeRequest, GetCounterRequest, IncrementRequest};
use rpcnet::RpcConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Concurrent Demo Client (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_server_name("localhost");

    let server_addr: SocketAddr = "127.0.0.1:8083".parse()?;

    println!("Connecting to concurrent demo service at {}", server_addr);
    let client = ConcurrentDemoClient::connect(server_addr, config.clone()).await?;
    println!("Connected successfully!");

    // Test computation
    println!("\n--- Testing CPU-Intensive Computation ---");
    let compute_req = ComputeRequest {
        task_id: "task_1".to_string(),
        iterations: 1_000_000,
    };
    match client.compute(compute_req).await {
        Ok(response) => {
            println!(
                "Task {} completed in {}ms with result {}",
                response.task_id, response.duration_ms, response.result
            );
        }
        Err(e) => println!("Computation failed: {}", e),
    }

    // Test async task
    println!("\n--- Testing Async Task ---");
    let async_req = AsyncTaskRequest {
        task_id: "async_1".to_string(),
        delay_ms: 2000, // 2 seconds
    };
    println!("Starting async task with 2 second delay...");
    match client.async_task(async_req).await {
        Ok(response) => {
            println!(
                "Async task {} completed at timestamp {}",
                response.task_id, response.completed_at
            );
        }
        Err(e) => println!("Async task failed: {}", e),
    }

    // Test shared counter
    println!("\n--- Testing Shared Counter ---");

    // Get initial value
    match client.get_counter(GetCounterRequest).await {
        Ok(response) => println!("Initial counter value: {}", response.value),
        Err(e) => println!("Failed to get counter: {}", e),
    }

    // Increment counter
    let inc_req = IncrementRequest { amount: 5 };
    match client.increment(inc_req).await {
        Ok(response) => println!("Counter after increment: {}", response.new_value),
        Err(e) => println!("Failed to increment: {}", e),
    }

    // Increment again
    let inc_req = IncrementRequest { amount: 10 };
    match client.increment(inc_req).await {
        Ok(response) => println!("Counter after second increment: {}", response.new_value),
        Err(e) => println!("Failed to increment: {}", e),
    }

    // Test concurrent operations
    println!("\n--- Testing Concurrent Operations ---");
    let mut handles = Vec::new();

    for _i in 0..5 {
        let client_clone = ConcurrentDemoClient::connect(server_addr, config.clone()).await?;
        let handle = tokio::spawn(async move {
            let req = IncrementRequest { amount: 1 };
            client_clone.increment(req).await
        });
        handles.push(handle);
    }

    // Wait for all concurrent increments
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await? {
            Ok(response) => println!(
                "Concurrent increment {}: counter = {}",
                i, response.new_value
            ),
            Err(e) => println!("Concurrent increment {} failed: {}", i, e),
        }
    }

    // Final counter value
    match client.get_counter(GetCounterRequest).await {
        Ok(response) => println!("Final counter value: {}", response.value),
        Err(e) => println!("Failed to get final counter: {}", e),
    }

    println!("\nAll tests completed!");
    Ok(())
}
