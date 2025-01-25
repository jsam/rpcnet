use criterion::{criterion_group, criterion_main, Criterion};
use std::net::SocketAddr;

use std::time::Duration;
use tokio::runtime::Runtime;

use rpcnet::{
    RpcClient, RpcConfig, RpcError, RpcServer
};

// ==========================
//   Benchmark Setup
// ==========================
fn test_config() -> RpcConfig {
    // Adjust paths as needed
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(30))
}

async fn start_server() -> Result<SocketAddr, RpcError> {
    let mut server = RpcServer::new(test_config());
    
    // Register echo handler
    server.register("echo", |params| async move {
        Ok(params) // Simple echo handler
    }).await;

    // Start the server in a separate task
    let ser = server.bind()?;
    let addr = ser.local_addr()?;
    
    let mut server_clone = server.clone();
    tokio::spawn(async move {
        let _ = server_clone.start(ser).await.expect("server vroom vroom");
    });

    Ok(addr)
}

// ==========================
//  The Benchmark Function
// ==========================
fn bench_rpc_echo(c: &mut Criterion) {
    // We'll use a single tokio runtime, so each iteration is just the call overhead,
    // not the overhead of spinning up the runtime or server each time.
    let rt = Runtime::new().unwrap();

    // 1) Start the server ONCE outside the measured loop:
    let addr = rt.block_on(async {
        println!("starting server");
        start_server().await.expect("server should start")
    });

    // 2) Create a single client that we'll reuse:
    let client = rt.block_on(async {
        println!("connecting to: {addr}");
        RpcClient::connect(addr, test_config()).await.expect("client connect")
    });

    // We'll do repeated calls in the benchmark
    c.bench_function("rpc echo call", |b| {
        // Using Criterion's `iter_custom` lets us do async in a single block
        // and measure the total time for N calls.
        b.iter_custom(|iterations| {
            let start = std::time::Instant::now();

            // Run all calls in a single block_on
            rt.block_on(async {
                for _ in 0..iterations {
                    // Minimal payload
                    let data = b"hello".to_vec();
                    let _ = client.call("echo", data).await.unwrap();
                }
            });

            // Return how long it took
            start.elapsed()
        });
    });

}

// Register the benchmark group + main entry
//criterion_group!(benches, bench_rpc_echo);
criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = bench_rpc_echo
}

criterion_main!(benches);
