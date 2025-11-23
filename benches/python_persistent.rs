#![allow(clippy::all)]
#![allow(warnings)]

//! Benchmark: Python Persistent Client with Connection Reuse
//!
//! Measures REALISTIC Python client performance by:
//! - Starting a single Python process (not subprocess per request)
//! - Reusing QUIC connections (production pattern)
//! - Making many requests over same connection
//! - Testing with varying concurrency levels
//!
//! This provides accurate production performance numbers.
//!
//! Run with: cargo bench --bench python_persistent --features python

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};

// Test configurations
const PAYLOAD_SIZES: &[usize] = &[100, 1_024, 10_240];
const CONCURRENCY_LEVELS: &[usize] = &[1, 10, 50];

/// Create a test Rust server that handles MessagePack-serialized requests
async fn setup_rust_server(port: u16) -> Result<SocketAddr, RpcError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let config = RpcConfig::new("certs/test_cert.pem", &bind_addr)
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);

    // Echo handler with MessagePack
    server
        .register("echo", |data: Vec<u8>| async move {
            let value: HashMap<String, Vec<u8>> = rmp_serde::from_slice(&data)
                .map_err(|e| RpcError::InternalError(format!("Deser: {}", e)))?;
            rmp_serde::to_vec(&value).map_err(|e| RpcError::InternalError(format!("Ser: {}", e)))
        })
        .await;

    let quic_server = server.bind()?;
    let addr = quic_server.local_addr()?;

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone
            .start(quic_server)
            .await
            .expect("Server failed");
    });

    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(addr)
}

/// Python script for persistent client benchmark
fn create_persistent_client_script(port: u16) -> String {
    format!(
        r#"
import asyncio
import sys
import time
import json
sys.path.insert(0, 'target/release')

import _rpcnet

async def main():
    # Connect once and reuse
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="0.0.0.0:0",
        server_name="localhost"
    )

    client = await _rpcnet.RpcClient.connect("127.0.0.1:{}", config)

    # Signal ready
    print("READY", flush=True)

    # Process benchmark commands from stdin
    for line in sys.stdin:
        cmd = json.loads(line.strip())

        if cmd["action"] == "bench":
            payload_size = cmd["payload_size"]
            num_requests = cmd["num_requests"]

            # Create payload
            payload = {{"data": list(b"x" * payload_size)}}
            serialized = _rpcnet.python_to_msgpack_py(payload)

            # Warmup
            for _ in range(5):
                await client.call("echo", serialized)

            # Benchmark
            start = time.perf_counter()
            for _ in range(num_requests):
                await client.call("echo", serialized)
            elapsed = time.perf_counter() - start

            # Report results
            rps = num_requests / elapsed
            avg_latency_us = (elapsed / num_requests) * 1_000_000
            result = {{
                "rps": rps,
                "latency_us": avg_latency_us,
                "total_time": elapsed
            }}
            print(json.dumps(result), flush=True)

        elif cmd["action"] == "exit":
            break

if __name__ == "__main__":
    asyncio.run(main())
"#,
        port
    )
}

/// Persistent Python client process
struct PersistentPythonClient {
    process: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl PersistentPythonClient {
    fn start(port: u16) -> Result<Self, String> {
        let script = create_persistent_client_script(port);

        // Try uv run first, fallback to python3
        let mut cmd = Command::new("uv");
        cmd.args(&["run", "python3", "-c", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut process = cmd
            .spawn()
            .or_else(|_| {
                Command::new("python3")
                    .arg("-c")
                    .arg(&script)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
            })
            .map_err(|e| format!("Failed to spawn Python: {}", e))?;

        let stdin = process.stdin.take().unwrap();
        let stdout = BufReader::new(process.stdout.take().unwrap());

        let mut client = PersistentPythonClient {
            process,
            stdin,
            stdout,
        };

        // Wait for READY signal
        client.wait_ready()?;

        Ok(client)
    }

    fn wait_ready(&mut self) -> Result<(), String> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read READY: {}", e))?;

        if !line.trim().starts_with("READY") {
            return Err(format!("Expected READY, got: {}", line));
        }

        Ok(())
    }

    fn run_benchmark(
        &mut self,
        payload_size: usize,
        num_requests: usize,
    ) -> Result<(f64, f64), String> {
        // Send command
        let cmd = serde_json::json!({
            "action": "bench",
            "payload_size": payload_size,
            "num_requests": num_requests
        });

        writeln!(self.stdin, "{}", cmd.to_string())
            .map_err(|e| format!("Failed to write command: {}", e))?;

        self.stdin
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        // Read result
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read result: {}", e))?;

        let result: serde_json::Value = serde_json::from_str(&line.trim())
            .map_err(|e| format!("Failed to parse result: {}", e))?;

        let rps = result["rps"].as_f64().unwrap();
        let latency_us = result["latency_us"].as_f64().unwrap();

        Ok((rps, latency_us))
    }

    fn shutdown(mut self) -> Result<(), String> {
        let cmd = serde_json::json!({"action": "exit"});
        writeln!(self.stdin, "{}", cmd.to_string()).ok();
        self.stdin.flush().ok();

        // Give it a moment to exit gracefully
        std::thread::sleep(Duration::from_millis(100));

        self.process.kill().ok();
        Ok(())
    }
}

/// Benchmark: Persistent Python client with connection reuse
fn bench_persistent_python(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Check if Python bindings available
    let check_uv = Command::new("uv")
        .args(&["run", "python3", "-c", "import _rpcnet"])
        .output();
    let check_system = Command::new("python3")
        .arg("-c")
        .arg("import _rpcnet")
        .output();

    let has_bindings = (check_uv.is_ok() && check_uv.unwrap().status.success())
        || (check_system.is_ok() && check_system.unwrap().status.success());

    if !has_bindings {
        println!("⚠️  Skipping persistent Python benchmark: Python bindings not built");
        println!("   Run: maturin develop --features python --release");
        return;
    }

    let mut group = c.benchmark_group("python_persistent");
    group.sample_size(30); // Fewer samples since it's more stable

    for &size in PAYLOAD_SIZES {
        let port = 20000 + (size as u16 % 100);

        // Start server
        let addr = runtime.block_on(setup_rust_server(port)).unwrap();
        let actual_port = addr.port();

        // Start persistent Python client
        let mut client = match PersistentPythonClient::start(actual_port) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to start Python client: {}", e);
                continue;
            }
        };

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                // Make 100 requests per iteration to amortize measurement overhead
                client.run_benchmark(size, 100).unwrap()
            });
        });

        client.shutdown().ok();
    }

    group.finish();
}

/// Benchmark: Compare Rust vs Persistent Python
fn bench_rust_vs_python(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Check Python availability
    let check_uv = Command::new("uv")
        .args(&["run", "python3", "-c", "import _rpcnet"])
        .output();
    let check_system = Command::new("python3")
        .arg("-c")
        .arg("import _rpcnet")
        .output();

    let has_python = (check_uv.is_ok() && check_uv.unwrap().status.success())
        || (check_system.is_ok() && check_system.unwrap().status.success());

    if !has_python {
        println!("⚠️  Skipping comparison: Python bindings not available");
        return;
    }

    let mut group = c.benchmark_group("rust_vs_python_persistent");
    let test_size = 1_024;

    // Rust baseline
    let rust_addr = runtime.block_on(setup_rust_server(20100)).unwrap();

    group.bench_function("rust_client_reused_connection", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
                    .with_server_name("localhost");

                let client = RpcClient::connect(rust_addr, config).await.unwrap();

                // Make 100 requests with connection reuse
                for _ in 0..100 {
                    let mut payload = HashMap::new();
                    payload.insert("data".to_string(), vec![0u8; test_size]);
                    let data = rmp_serde::to_vec(&payload).unwrap();
                    client.call("echo", data).await.unwrap();
                }
            })
        });
    });

    // Python with connection reuse
    let python_addr = runtime.block_on(setup_rust_server(20101)).unwrap();
    let mut python_client = PersistentPythonClient::start(python_addr.port()).unwrap();

    group.bench_function("python_client_reused_connection", |b| {
        b.iter(|| {
            python_client.run_benchmark(test_size, 100).unwrap();
        });
    });

    python_client.shutdown().ok();
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(15));
    targets = bench_persistent_python, bench_rust_vs_python
}

criterion_main!(benches);
