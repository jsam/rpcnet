// Enable high-performance memory allocator when perf feature is active
#[cfg(feature = "perf")]
use jemallocator::Jemalloc;

#[cfg(feature = "perf")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId, Throughput};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Barrier;

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcServer};

// ==========================
//   Benchmark Setup
// ==========================

/// Create an optimized Tokio runtime for maximum performance
fn create_optimized_runtime() -> Runtime {
    #[cfg(feature = "perf")]
    {
        Builder::new_multi_thread()
            .worker_threads(num_cpus::get()) // One thread per CPU core
            .max_blocking_threads(512) // Higher blocking thread limit
            .thread_stack_size(2 * 1024 * 1024) // 2MB stack size
            .thread_name("rpc-perf-worker")
            .thread_keep_alive(Duration::from_secs(60)) // Keep threads alive longer
            .global_queue_interval(31) // Reduce global queue polling
            .event_interval(61) // Reduce event polling overhead
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
    // Optimized config for maximum performance in benchmarks
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(50)) // Faster keep-alive for benchmarks
}

async fn start_server() -> Result<SocketAddr, RpcError> {
    let mut server = RpcServer::new(test_config());

    // Register echo handler (simple echo)
    server
        .register("echo", |params| async move {
            Ok(params)
        })
        .await;

    // Register compute handler (CPU-intensive)
    server
        .register("compute", |params| async move {
            let iterations: u32 = bincode::deserialize(&params)
                .map_err(RpcError::SerializationError)?;
            
            // Simulate some work
            let mut result = 0u64;
            for i in 0..iterations {
                result = result.wrapping_add(i as u64);
            }
            
            bincode::serialize(&result)
                .map_err(RpcError::SerializationError)
        })
        .await;

    // Register large data handler
    server
        .register("large_data", |params| async move {
            // Return data of the same size as input
            Ok(params)
        })
        .await;

    // Start the server in a separate task
    let ser = server.bind()?;
    let addr = ser.local_addr()?;

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone.start(ser).await.expect("Server failed to start");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    Ok(addr)
}

// ==========================
//  Benchmark Functions
// ==========================

/// Benchmark simple echo calls with various payload sizes
fn bench_echo_throughput(c: &mut Criterion) {
    let rt = create_optimized_runtime();
    
    // Start server once
    let addr = rt.block_on(async {
        start_server().await.expect("Server should start")
    });
    
    // Create client
    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });
    
    let mut group = c.benchmark_group("echo_throughput");
    
    // Test different payload sizes
    let sizes = [1, 100, 1_000, 10_000, 100_000]; // bytes
    
    for size in sizes {
        let data = vec![0u8; size];
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("payload_size", size),
            &size,
            |b, _| {
                b.iter_custom(|iterations| {
                    let start = Instant::now();
                    
                    rt.block_on(async {
                        for _ in 0..iterations {
                            client.call("echo", data.clone()).await.unwrap();
                        }
                    });
                    
                    start.elapsed()
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent requests using connection pooling
fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = create_optimized_runtime();
    
    let addr = rt.block_on(async {
        start_server().await.expect("Server should start")
    });
    
    // Pre-create a fixed pool of clients to avoid connection limits
    let client_pool: Arc<Vec<RpcClient>> = rt.block_on(async {
        let mut clients = Vec::new();
        for _ in 0..4 { // Only 4 concurrent connections
            let client = RpcClient::connect(addr, test_config())
                .await
                .expect("Client connection failed");
            clients.push(client);
        }
        Arc::new(clients)
    });
    
    let mut group = c.benchmark_group("concurrent_requests");
    group.sample_size(30);
    
    let concurrency_patterns = [
        ("single_client", 1),
        ("dual_client", 2), 
        ("quad_client", 4),
        ("round_robin", 4), // Use all 4 clients in round-robin
    ];
    
    for (name, client_count) in concurrency_patterns {
        group.throughput(Throughput::Elements(1));
        group.bench_function(name, |b| {
            let pool = client_pool.clone();
            b.iter_custom(|iterations| {
                let start = Instant::now();
                
                rt.block_on(async {
                    let data = b"concurrent test".to_vec();
                    
                    if name == "round_robin" {
                        // Round-robin through all clients
                        let mut tasks = Vec::new();
                        for i in 0..iterations {
                            let client = &pool[i as usize % client_count];
                            let data = data.clone();
                            let task = async move {
                                client.call("echo", data).await.unwrap()
                            };
                            tasks.push(task);
                        }
                        futures::future::join_all(tasks).await;
                    } else {
                        // Parallel execution with limited clients
                        let requests_per_client = iterations / client_count as u64;
                        let client_tasks: Vec<_> = (0..client_count).map(|i| {
                            let client = &pool[i];
                            let data = data.clone();
                            async move {
                                for _ in 0..requests_per_client {
                                    client.call("echo", data.clone()).await.unwrap();
                                }
                            }
                        }).collect();
                        
                        futures::future::join_all(client_tasks).await;
                    }
                });
                
                elapsed_with_stats(start, iterations)
            });
        });
    }
    
    group.finish();
}

/// Benchmark CPU-intensive operations
fn bench_compute_operations(c: &mut Criterion) {
    let rt = create_optimized_runtime();
    
    let addr = rt.block_on(async {
        start_server().await.expect("Server should start")
    });
    
    let client = rt.block_on(async {
        RpcClient::connect(addr, test_config())
            .await
            .expect("Client connection failed")
    });
    
    let mut group = c.benchmark_group("compute_operations");
    
    let work_levels = [1_000, 10_000, 100_000]; // computation iterations
    
    for work in work_levels {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("compute_iterations", work),
            &work,
            |b, &work| {
                b.iter_custom(|iterations| {
                    let start = Instant::now();
                    
                    rt.block_on(async {
                        for _ in 0..iterations {
                            let params = bincode::serialize(&work).unwrap();
                            client.call("compute", params).await.unwrap();
                        }
                    });
                    
                    start.elapsed()
                });
            },
        );
    }
    
    group.finish();
}

/// High-frequency benchmark to show maximum throughput
fn bench_max_throughput(c: &mut Criterion) {
    let rt = create_optimized_runtime();
    
    let addr = rt.block_on(async {
        start_server().await.expect("Server should start")
    });
    
    // Create a small, fixed pool of clients to avoid resource exhaustion
    let clients: Arc<Vec<RpcClient>> = rt.block_on(async {
        let mut clients = Vec::new();
        for _ in 0..4 { // Only 4 connections to avoid OS limits
            let client = RpcClient::connect(addr, test_config())
                .await
                .expect("Client connection failed");
            clients.push(client);
        }
        Arc::new(clients)
    });
    
    let mut group = c.benchmark_group("max_throughput");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));
    
    // Sequential baseline - single client, one request at a time
    group.throughput(Throughput::Elements(1));
    group.bench_function("sequential", |b| {
        let clients = clients.clone();
        b.iter_custom(|iterations| {
            let start = Instant::now();
            
            rt.block_on(async {
                let client = &clients[0];
                let data = vec![1u8];
                
                for _ in 0..iterations {
                    client.call("echo", data.clone()).await.unwrap();
                }
            });
            
            elapsed_with_stats(start, iterations)
        });
    });

    // Pipelined - single client, all requests concurrent
    group.throughput(Throughput::Elements(1));
    group.bench_function("pipelined", |b| {
        let clients = clients.clone();
        b.iter_custom(|iterations| {
            let start = Instant::now();
            
            rt.block_on(async {
                let client = &clients[0];
                let data = vec![1u8];
                let mut tasks = Vec::new();
                
                // Launch all requests concurrently on single connection
                for _ in 0..iterations {
                    let task = client.call("echo", data.clone());
                    tasks.push(task);
                }
                
                futures::future::join_all(tasks).await;
            });
            
            elapsed_with_stats(start, iterations)
        });
    });

    // Multi-client parallel - distribute across all clients
    group.throughput(Throughput::Elements(1));
    group.bench_function("parallel", |b| {
        let clients = clients.clone();
        b.iter_custom(|iterations| {
            let start = Instant::now();
            
            rt.block_on(async {
                let data = vec![1u8];
                let requests_per_client = iterations / clients.len() as u64;
                
                let client_tasks: Vec<_> = clients.iter().map(|client| {
                    let data = data.clone();
                    async move {
                        let mut tasks = Vec::new();
                        for _ in 0..requests_per_client {
                            let task = client.call("echo", data.clone());
                            tasks.push(task);
                        }
                        futures::future::join_all(tasks).await
                    }
                }).collect();
                
                futures::future::join_all(client_tasks).await;
            });
            
            elapsed_with_stats(start, iterations)
        });
    });

    // Sustainable throughput test - longer duration, realistic load
    group.throughput(Throughput::Elements(1));
    group.bench_function("sustainable", |b| {
        let clients = clients.clone();
        b.iter_custom(|iterations| {
            let start = Instant::now();
            
            rt.block_on(async {
                let data = vec![1u8];
                
                // Use round-robin to distribute load evenly
                let mut tasks = Vec::new();
                for i in 0..iterations {
                    let client = &clients[i as usize % clients.len()];
                    let data = data.clone();
                    let task = async move {
                        client.call("echo", data).await.unwrap()
                    };
                    tasks.push(task);
                }
                
                futures::future::join_all(tasks).await;
            });
            
            elapsed_with_stats(start, iterations)
        });
    });
    
    group.finish();
}

fn elapsed_with_stats(start: Instant, iterations: u64) -> Duration {
    let elapsed = start.elapsed();
    
    // Print throughput statistics for significant runs only
    let requests_per_second = iterations as f64 / elapsed.as_secs_f64();
    
    if iterations >= 1000 { // Only print for substantial runs
        println!("\n📊 Performance Metrics:");
        println!("   Requests: {} in {:.3}s", iterations, elapsed.as_secs_f64());
        println!("   Throughput: {:.0} req/s", requests_per_second);
        
        // Contextual performance ratings
        let (emoji, rating, context) = if requests_per_second > 80_000.0 {
            ("🚀", "Exceptional", "Outstanding for encrypted RPC")
        } else if requests_per_second > 50_000.0 {
            ("⚡", "Excellent", "Very high performance")  
        } else if requests_per_second > 25_000.0 {
            ("✅", "Very Good", "Strong QUIC+TLS performance")
        } else if requests_per_second > 10_000.0 {
            ("📊", "Good", "Solid encrypted throughput")
        } else {
            ("📈", "Baseline", "Standard performance")
        };
        
        println!("   {} {}: {:.1}K req/s - {}", emoji, rating, requests_per_second / 1000.0, context);
        
        // Additional context for high performance
        if requests_per_second > 40_000.0 {
            println!("   🔒 Impressive throughput considering full QUIC+TLS encryption overhead");
        }
    }
    
    elapsed
}

// Register all benchmarks
criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = 
        bench_echo_throughput,
        bench_concurrent_requests,
        bench_compute_operations,
        bench_max_throughput
}

criterion_main!(benches);
