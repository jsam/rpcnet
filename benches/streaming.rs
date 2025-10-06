#![allow(clippy::all)]
#![allow(warnings)]
#[cfg(feature = "perf")]
use jemallocator::Jemalloc;

#[cfg(feature = "perf")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use futures::StreamExt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};

fn create_optimized_runtime() -> Runtime {
    #[cfg(feature = "perf")]
    {
        Builder::new_multi_thread()
            .worker_threads(num_cpus::get())
            .max_blocking_threads(512)
            .thread_stack_size(2 * 1024 * 1024)
            .thread_name("rpc-streaming-worker")
            .thread_keep_alive(Duration::from_secs(60))
            .global_queue_interval(31)
            .event_interval(61)
            .enable_all()
            .build()
            .expect("Failed to create optimized Tokio runtime")
    }
    #[cfg(not(feature = "perf"))]
    {
        Runtime::new().expect("Failed to create Tokio runtime")
    }
}

fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(50))
}

async fn start_streaming_server() -> Result<SocketAddr, RpcError> {
    let mut server = RpcServer::new(test_config());

    server
        .register_streaming("echo_stream", |request_stream| async move {
            async_stream::stream! {
                let mut request_stream = Box::pin(request_stream);
                while let Some(data) = request_stream.next().await {
                    yield Ok(data);
                }
            }
        })
        .await;

    server
        .register_streaming("burst_tokens", |request_stream| async move {
            async_stream::stream! {
                let mut request_stream = Box::pin(request_stream);
                if let Some(request_data) = request_stream.next().await {
                    let count: u32 = bincode::deserialize(&request_data)
                        .unwrap_or(100);

                    for i in 0..count {
                        let token = format!("token-{}", i);
                        if let Ok(bytes) = bincode::serialize(&token) {
                            yield Ok(bytes);
                        }
                    }
                }
            }
        })
        .await;

    server
        .register_streaming("concurrent_echo", |request_stream| async move {
            async_stream::stream! {
                let mut request_stream = Box::pin(request_stream);
                while let Some(data) = request_stream.next().await {
                    yield Ok(data);
                }
            }
        })
        .await;

    let ser = server.bind()?;
    let addr = ser.local_addr()?;

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone
            .start(ser)
            .await
            .expect("Server failed to start");
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(addr)
}

fn bench_bidirectional_streaming_throughput(c: &mut Criterion) {
    let rt = create_optimized_runtime();

    let addr = rt.block_on(async { start_streaming_server().await.expect("Server should start") });

    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });

    let mut group = c.benchmark_group("bidirectional_streaming");
    group.sample_size(30);

    let stream_sizes = [10, 50, 100, 500];

    for size in stream_sizes {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("echo_messages", size),
            &size,
            |b, &size| {
                b.iter_custom(|iterations| {
                    let start = Instant::now();

                    rt.block_on(async {
                        for _ in 0..iterations {
                            let messages: Vec<Vec<u8>> = (0..size)
                                .map(|i| format!("message-{}", i).into_bytes())
                                .collect();

                            let request_stream = futures::stream::iter(messages);

                            let response_stream = client
                                .call_streaming("echo_stream", request_stream)
                                .await
                                .unwrap();

                            let mut response_stream = Box::pin(response_stream);
                            let mut count = 0;
                            while let Some(Ok(_response)) = response_stream.next().await {
                                count += 1;
                                if count >= size {
                                    break;
                                }
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

fn bench_streaming_token_burst(c: &mut Criterion) {
    let rt = create_optimized_runtime();

    let addr = rt.block_on(async { start_streaming_server().await.expect("Server should start") });

    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });

    let mut group = c.benchmark_group("streaming_token_burst");
    group.sample_size(30);

    let token_counts = [100, 500, 1000];

    for count in token_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("tokens", count), &count, |b, &count| {
            b.iter_custom(|iterations| {
                let start = Instant::now();

                rt.block_on(async {
                    for _ in 0..iterations {
                        let request = bincode::serialize(&count).unwrap();
                        let request_stream = futures::stream::iter(vec![request]);

                        let response_stream = client
                            .call_streaming("burst_tokens", request_stream)
                            .await
                            .unwrap();

                        let mut response_stream = Box::pin(response_stream);
                        let mut received = 0;
                        while let Some(Ok(_token)) = response_stream.next().await {
                            received += 1;
                            if received >= count {
                                break;
                            }
                        }
                    }
                });

                elapsed_with_stats(start, iterations, count as u64)
            });
        });
    }

    group.finish();
}

fn bench_concurrent_streaming(c: &mut Criterion) {
    let rt = create_optimized_runtime();

    let addr = rt.block_on(async { start_streaming_server().await.expect("Server should start") });

    let clients: Arc<Vec<RpcClient>> = rt.block_on(async {
        let mut clients = Vec::new();
        for _ in 0..4 {
            let client = RpcClient::connect(addr, test_config())
                .await
                .expect("Client connection failed");
            clients.push(client);
        }
        Arc::new(clients)
    });

    let mut group = c.benchmark_group("concurrent_streaming");
    group.sample_size(20);

    let patterns = [("single_stream", 1), ("dual_stream", 2), ("quad_stream", 4)];

    for (name, client_count) in patterns {
        group.throughput(Throughput::Elements(100));
        group.bench_function(name, |b| {
            let clients = clients.clone();
            b.iter_custom(|iterations| {
                let start = Instant::now();

                rt.block_on(async {
                    let messages_per_stream = 100;

                    for _ in 0..iterations {
                        let stream_tasks: Vec<_> = (0..client_count)
                            .map(|i| {
                                let client = &clients[i];
                                async move {
                                    let messages: Vec<Vec<u8>> = (0..messages_per_stream)
                                        .map(|j| format!("msg-{}", j).into_bytes())
                                        .collect();

                                    let request_stream = futures::stream::iter(messages);

                                    let response_stream = client
                                        .call_streaming("concurrent_echo", request_stream)
                                        .await
                                        .unwrap();

                                    let mut response_stream = Box::pin(response_stream);
                                    let mut count = 0;
                                    while let Some(Ok(_)) = response_stream.next().await {
                                        count += 1;
                                        if count >= messages_per_stream {
                                            break;
                                        }
                                    }
                                }
                            })
                            .collect();

                        futures::future::join_all(stream_tasks).await;
                    }
                });

                elapsed_with_stats(start, iterations, client_count as u64 * 100)
            });
        });
    }

    group.finish();
}

fn bench_lockfree_streaming_latency(c: &mut Criterion) {
    let rt = create_optimized_runtime();

    let addr = rt.block_on(async { start_streaming_server().await.expect("Server should start") });

    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });

    let mut group = c.benchmark_group("lockfree_streaming_latency");
    group.sample_size(50);

    println!("\n🌐 Network Latency Testing Notes:");
    println!("   Current: Localhost (< 0.1ms baseline)");
    println!("   For realistic testing:");
    println!("   1. Deploy server to cloud: cargo run --release --bin <server>");
    println!("   2. Run client locally with BENCHMARK_SERVER_ADDR=<ip>:<port>");
    println!("   3. Use 'tc qdisc' to simulate network delay:");
    println!("      sudo tc qdisc add dev lo root netem delay 50ms");
    println!("   4. Cross-region cloud instances for real internet latency");

    group.bench_function("single_message_roundtrip", |b| {
        b.iter_custom(|iterations| {
            let mut latencies = Vec::with_capacity(iterations as usize);

            rt.block_on(async {
                for _ in 0..iterations {
                    let iteration_start = Instant::now();

                    let message = b"latency test".to_vec();
                    let request_stream = futures::stream::iter(vec![message]);

                    let response_stream = client
                        .call_streaming("echo_stream", request_stream)
                        .await
                        .unwrap();

                    let mut response_stream = Box::pin(response_stream);
                    if let Some(Ok(_response)) = response_stream.next().await {
                        latencies.push(iteration_start.elapsed());
                    }
                }
            });

            print_latency_stats(&latencies, "Single Message Roundtrip");
            latencies.iter().sum()
        });
    });

    group.bench_function("pipelined_messages", |b| {
        b.iter_custom(|iterations| {
            let start = Instant::now();

            rt.block_on(async {
                for _ in 0..iterations {
                    let messages: Vec<Vec<u8>> =
                        (0..10).map(|i| format!("msg-{}", i).into_bytes()).collect();

                    let request_stream = futures::stream::iter(messages);

                    let response_stream = client
                        .call_streaming("echo_stream", request_stream)
                        .await
                        .unwrap();

                    let mut response_stream = Box::pin(response_stream);
                    let mut count = 0;
                    while let Some(Ok(_)) = response_stream.next().await {
                        count += 1;
                        if count >= 10 {
                            break;
                        }
                    }
                }
            });

            start.elapsed()
        });
    });

    group.finish();
}

fn print_latency_stats(latencies: &[Duration], label: &str) {
    if latencies.is_empty() {
        return;
    }

    let mut sorted_latencies = latencies.to_vec();
    sorted_latencies.sort();

    let total: Duration = sorted_latencies.iter().sum();
    let mean = total / sorted_latencies.len() as u32;

    let p50 = sorted_latencies[sorted_latencies.len() * 50 / 100];
    let p90 = sorted_latencies[sorted_latencies.len() * 90 / 100];
    let p95 = sorted_latencies[sorted_latencies.len() * 95 / 100];
    let p99 = sorted_latencies[sorted_latencies.len() * 99 / 100];
    let min = sorted_latencies[0];
    let max = sorted_latencies[sorted_latencies.len() - 1];

    println!("\n⏱️  Latency Statistics - {}:", label);
    println!("   Samples: {}", latencies.len());
    println!("   Mean:    {:>8.3} ms", mean.as_secs_f64() * 1000.0);
    println!("   Min:     {:>8.3} ms", min.as_secs_f64() * 1000.0);
    println!("   P50:     {:>8.3} ms", p50.as_secs_f64() * 1000.0);
    println!("   P90:     {:>8.3} ms", p90.as_secs_f64() * 1000.0);
    println!("   P95:     {:>8.3} ms", p95.as_secs_f64() * 1000.0);
    println!("   P99:     {:>8.3} ms", p99.as_secs_f64() * 1000.0);
    println!("   Max:     {:>8.3} ms", max.as_secs_f64() * 1000.0);

    let (emoji, rating) = if mean.as_micros() < 500 {
        ("🚀", "Exceptional")
    } else if mean.as_micros() < 1000 {
        ("⚡", "Excellent")
    } else if mean.as_micros() < 2000 {
        ("✅", "Very Good")
    } else if mean.as_micros() < 5000 {
        ("📊", "Good")
    } else {
        ("📈", "Baseline")
    };

    println!(
        "   {} {}: {:.3} ms mean latency",
        emoji,
        rating,
        mean.as_secs_f64() * 1000.0
    );
}

fn elapsed_with_stats(start: Instant, iterations: u64, items_per_iteration: u64) -> Duration {
    let elapsed = start.elapsed();
    let total_items = iterations * items_per_iteration;
    let items_per_second = total_items as f64 / elapsed.as_secs_f64();

    if total_items >= 1000 {
        println!("\n📊 Streaming Performance:");
        println!(
            "   Messages: {} in {:.3}s",
            total_items,
            elapsed.as_secs_f64()
        );
        println!("   Throughput: {:.0} msg/s", items_per_second);

        let (emoji, rating) = if items_per_second > 100_000.0 {
            ("🚀", "Exceptional")
        } else if items_per_second > 50_000.0 {
            ("⚡", "Excellent")
        } else if items_per_second > 25_000.0 {
            ("✅", "Very Good")
        } else if items_per_second > 10_000.0 {
            ("📊", "Good")
        } else {
            ("📈", "Baseline")
        };

        println!(
            "   {} {}: {:.1}K msg/s - Lock-free bidirectional streaming",
            emoji,
            rating,
            items_per_second / 1000.0
        );
    }

    elapsed
}

criterion_group! {
    name = streaming_benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets =
        bench_bidirectional_streaming_throughput,
        bench_streaming_token_burst,
        bench_concurrent_streaming,
        bench_lockfree_streaming_latency
}

criterion_main!(streaming_benches);
