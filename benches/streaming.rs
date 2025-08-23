use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId, Throughput};
use futures::StreamExt;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};

fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(50))
}

/// Start a server with streaming capabilities
async fn start_server() -> std::net::SocketAddr {
    let mut server = RpcServer::new(test_config());

    // Register streaming handler for benchmarks
    server.register_streaming("benchmark_stream", |_request_stream| async move {
        Box::pin(async_stream::stream! {
            // Stream 100 responses for consistent benchmarking
            for i in 0..100 {
                let data = format!("response_{}", i).into_bytes();
                yield Ok(data);
            }
        }) as std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<u8>, RpcError>> + Send>>
    }).await;

    let ser = server.bind().expect("Failed to bind server");
    let addr = ser.local_addr().expect("Failed to get server address");

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone.start(ser).await.expect("Server failed to start");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    addr
}

/// Benchmark server streaming responses
fn bench_streaming_responses(c: &mut Criterion) {
    let rt = Runtime::new().expect("Failed to create runtime");

    // Start server once
    let addr = rt.block_on(async { start_server().await });
    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });

    let mut group = c.benchmark_group("streaming_responses");

    // Test different numbers of responses to consume
    for response_count in [10, 50, 100] {
        group.throughput(Throughput::Elements(response_count));
        group.bench_with_input(
            BenchmarkId::new("responses", response_count),
            &response_count,
            |b, &response_count| {
                b.iter_custom(|iterations| {
                    let start = Instant::now();

                    rt.block_on(async {
                        for _ in 0..iterations {
                            // Call server streaming
                            let response_stream = client
                                .call_server_streaming("benchmark_stream", vec![0u8])
                                .await
                                .expect("Server streaming call failed");

                            // Collect the specified number of responses
                            let responses: Vec<_> = Box::pin(response_stream)
                                .take(response_count as usize)
                                .collect()
                                .await;

                            // Verify we got the expected number of responses
                            assert_eq!(responses.len(), response_count as usize);

                            // Verify all responses are Ok
                            for response in responses {
                                assert!(response.is_ok(), "All responses should be Ok");
                            }
                        }
                    });

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = streaming_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = bench_streaming_responses
);

criterion_main!(streaming_benches);