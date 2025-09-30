//! Calculator client using generated code.

#[path = "generated/calculator/mod.rs"]
mod calculator;

use calculator::client::CalculatorClient;
use calculator::{AddRequest, DivideRequest, MultiplyRequest, SubtractRequest};
use rpcnet::RpcConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Calculator Client (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0").with_server_name("localhost");

    let server_addr: SocketAddr = "127.0.0.1:8090".parse()?;

    println!("Connecting to calculator service at {}", server_addr);
    let client = CalculatorClient::connect(server_addr, config).await?;
    println!("Connected successfully!");

    // Test addition
    println!("\n--- Testing Addition ---");
    let add_req = AddRequest { a: 10, b: 20 };
    match client.add(add_req).await {
        Ok(response) => println!("10 + 20 = {}", response.result),
        Err(e) => println!("Addition failed: {}", e),
    }

    // Test subtraction
    println!("\n--- Testing Subtraction ---");
    let sub_req = SubtractRequest { a: 100, b: 30 };
    match client.subtract(sub_req).await {
        Ok(response) => println!("100 - 30 = {}", response.result),
        Err(e) => println!("Subtraction failed: {}", e),
    }

    // Test multiplication
    println!("\n--- Testing Multiplication ---");
    let mul_req = MultiplyRequest { a: 7, b: 8 };
    match client.multiply(mul_req).await {
        Ok(response) => println!("7 * 8 = {}", response.result),
        Err(e) => println!("Multiplication failed: {}", e),
    }

    // Test division
    println!("\n--- Testing Division ---");
    let div_req = DivideRequest {
        dividend: 100.0,
        divisor: 4.0,
    };
    match client.divide(div_req).await {
        Ok(response) => println!("100.0 / 4.0 = {}", response.result),
        Err(e) => println!("Division failed: {}", e),
    }

    // Test division by zero
    let div_req = DivideRequest {
        dividend: 100.0,
        divisor: 0.0,
    };
    match client.divide(div_req).await {
        Ok(response) => println!("100.0 / 0.0 = {} (shouldn't happen!)", response.result),
        Err(e) => println!("Division by zero handled correctly: {}", e),
    }

    // Test overflow
    println!("\n--- Testing Overflow ---");
    let add_req = AddRequest { a: i64::MAX, b: 1 };
    match client.add(add_req).await {
        Ok(response) => println!("{} + 1 = {}", i64::MAX, response.result),
        Err(e) => println!("Overflow handled correctly: {}", e),
    }

    println!("\nAll tests completed!");
    Ok(())
}
