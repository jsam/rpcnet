#![allow(clippy::all)]
#![allow(warnings)]

//! Benchmark: Rust Server ↔ Python Client Interop
//!
//! Measures the performance overhead of Python↔Rust RPC calls:
//! - MessagePack serialization/deserialization
//! - PyO3 bridge overhead
//! - Comparison with pure Rust↔Rust calls
//!
//! Run with: cargo bench --bench python_interop --features python

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};

// Test payload sizes
const PAYLOAD_SIZES: &[usize] = &[
    100,       // 100 bytes - small message
    1_024,     // 1 KB - typical request
    10_240,    // 10 KB - medium payload
    102_400,   // 100 KB - large payload
];

/// Create a test Rust server that handles MessagePack-serialized requests
async fn setup_rust_server(port: u16) -> Result<SocketAddr, RpcError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let config = RpcConfig::new("certs/test_cert.pem", &bind_addr)
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let mut server = RpcServer::new(config);

    // Echo handler - mirrors Rust benchmarks but with MessagePack serialization
    server
        .register("echo", |data: Vec<u8>| async move {
            // Deserialize from MessagePack
            let value: HashMap<String, Vec<u8>> = rmp_serde::from_slice(&data)
                .map_err(|e| RpcError::InternalError(format!("Deser failed: {}", e)))?;

            // Serialize back to MessagePack
            rmp_serde::to_vec(&value)
                .map_err(|e| RpcError::InternalError(format!("Ser failed: {}", e)))
        })
        .await;

    // Start server
    let quic_server = server.bind()?;
    let addr = quic_server.local_addr()?;

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone.start(quic_server).await.expect("Server failed");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(addr)
}

/// Create a Python client subprocess that makes RPC calls
fn create_python_client_script(port: u16, payload_size: usize, num_requests: usize) -> String {
    format!(
        r#"
import asyncio
import sys
import time
sys.path.insert(0, 'target/release')

import _rpcnet

async def benchmark():
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="0.0.0.0:0",
        server_name="localhost"
    )

    client = await _rpcnet.RpcClient.connect("127.0.0.1:{}", config)

    # Create payload - MessagePack needs dict, bytes go inside
    payload = {{"data": list(b"x" * {})}}  # Convert bytes to list of ints
    serialized = _rpcnet.python_to_msgpack_py(payload)

    # Warmup
    for _ in range(10):
        await client.call("echo", serialized)

    # Benchmark
    start = time.perf_counter()
    for _ in range({}):
        response = await client.call("echo", serialized)
    elapsed = time.perf_counter() - start

    # Output: requests_per_second,avg_latency_us
    rps = {} / elapsed
    avg_latency_us = (elapsed / {}) * 1_000_000
    print(f"{{rps:.2f}},{{avg_latency_us:.2f}}")

asyncio.run(benchmark())
"#,
        port, payload_size, num_requests, num_requests, num_requests
    )
}

/// Run Python client and parse results
fn run_python_benchmark(
    runtime: &Runtime,
    port: u16,
    payload_size: usize,
    num_requests: usize,
) -> (f64, f64) {
    let script = create_python_client_script(port, payload_size, num_requests);

    let output = runtime.block_on(async {
        tokio::task::spawn_blocking(move || {
            // Try uv run first (if using uv), fallback to system python
            let result = Command::new("uv")
                .args(&["run", "python3", "-c", &script])
                .output();

            if result.is_ok() {
                result.unwrap()
            } else {
                // Fallback to system python3
                Command::new("python3")
                    .arg("-c")
                    .arg(&script)
                    .output()
                    .expect("Failed to run Python benchmark")
            }
        })
        .await
        .unwrap()
    });

    if !output.status.success() {
        panic!(
            "Python benchmark failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = result.trim().split(',').collect();

    let rps: f64 = parts[0].parse().expect("Failed to parse RPS");
    let latency: f64 = parts[1].parse().expect("Failed to parse latency");

    (rps, latency)
}

/// Benchmark: Python client → Rust server with MessagePack
fn bench_python_to_rust(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Check if Python bindings are available
    // Try uv environment first, then system python
    let check_uv = Command::new("uv")
        .args(&["run", "python3", "-c", "import _rpcnet"])
        .output();

    let check_system = Command::new("python3")
        .arg("-c")
        .arg("import sys; sys.path.insert(0, 'target/release'); import _rpcnet")
        .output();

    let has_bindings = (check_uv.is_ok() && check_uv.unwrap().status.success())
        || (check_system.is_ok() && check_system.unwrap().status.success());

    if !has_bindings {
        println!("⚠️  Skipping Python benchmarks: Python bindings not built");
        println!("   Run: maturin develop --features python --release");
        println!("   Or: uv run maturin develop --features python --release");
        return;
    }

    let mut group = c.benchmark_group("python_to_rust");

    for &size in PAYLOAD_SIZES {
        let port = 19000 + (size as u16 % 100); // Unique port per size

        // Start Rust server
        let addr = runtime.block_on(setup_rust_server(port)).unwrap();
        let port = addr.port();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let (rps, latency_us) = run_python_benchmark(&runtime, port, size, 100);
                    (rps, latency_us)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Compare Rust↔Rust vs Python↔Rust
fn bench_comparison(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Check Python availability
    let check_uv = Command::new("uv")
        .args(&["run", "python3", "-c", "import _rpcnet"])
        .output();

    let check_system = Command::new("python3")
        .arg("-c")
        .arg("import sys; sys.path.insert(0, 'target/release'); import _rpcnet")
        .output();

    let has_python = (check_uv.is_ok() && check_uv.unwrap().status.success())
        || (check_system.is_ok() && check_system.unwrap().status.success());

    if !has_python {
        println!("⚠️  Skipping comparison benchmarks: Python bindings not available");
        return;
    }

    let mut group = c.benchmark_group("interop_comparison");
    let test_size = 1_024; // 1KB payload

    // Rust client → Rust server (bincode)
    let rust_addr = runtime.block_on(setup_rust_server(19100)).unwrap();

    group.bench_function("rust_to_rust_bincode", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
                    .with_server_name("localhost");

                let client = RpcClient::connect(rust_addr, config).await.unwrap();

                // Create MessagePack payload like Python would
                let mut payload = HashMap::new();
                payload.insert("data".to_string(), vec![0u8; test_size]);
                let data = rmp_serde::to_vec(&payload).unwrap();

                let _response = client.call("echo", data).await.unwrap();
            })
        });
    });

    // Python client → Rust server (MessagePack)
    let python_addr = runtime.block_on(setup_rust_server(19101)).unwrap();

    group.bench_function("python_to_rust_msgpack", |b| {
        b.iter(|| {
            let (rps, latency_us) =
                run_python_benchmark(&runtime, python_addr.port(), test_size, 100);
            (rps, latency_us)
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(50)  // Fewer samples since Python is slower
        .measurement_time(Duration::from_secs(10));
    targets = bench_python_to_rust, bench_comparison
}

criterion_main!(benches);
