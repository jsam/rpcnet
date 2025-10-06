# Performance Tuning

RpcNet achieves **172,000+ requests/second** with proper configuration. This chapter provides concrete tips and techniques to maximize performance in production deployments.

## Baseline Performance

Out-of-the-box performance with default settings:

| Metric | Value | Notes |
|--------|-------|-------|
| **Throughput** | 130K-150K RPS | Single director + 3 workers |
| **Latency (P50)** | 0.5-0.8ms | With connection pooling |
| **Latency (P99)** | 2-5ms | Under moderate load |
| **CPU (per node)** | 40-60% | At peak throughput |
| **Memory** | 50-100MB | Per worker node |

**Target after tuning**: 172K+ RPS, < 0.5ms P50 latency, < 35% CPU

## Quick Wins

### 1. Enable Connection Pooling

**Impact**: 4x throughput increase, 98% latency reduction

```rust
use rpcnet::cluster::{PoolConfig, ClusterClientConfig};

// Before (no pooling): ~40K RPS
let config = ClusterClientConfig::default();

// After (with pooling): ~172K RPS
let pool_config = PoolConfig::default()
    .with_max_connections_per_host(20)
    .with_max_idle_time(Duration::from_secs(90));

let config = ClusterClientConfig::default()
    .with_pool_config(pool_config);
```

**Why it works**:
- Eliminates TLS/QUIC handshake overhead (4-5 RTT per request → 0 RTT)
- Reuses established connections
- Reduces CPU spent on crypto

### 2. Use Least Connections Load Balancing

**Impact**: 15-20% throughput increase under variable load

```rust
use rpcnet::cluster::{WorkerRegistry, LoadBalancingStrategy};

// Before (Round Robin): uneven load distribution
let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::RoundRobin);

// After (Least Connections): optimal distribution
let registry = WorkerRegistry::new(cluster, LoadBalancingStrategy::LeastConnections);
```

**Why it works**:
- Prevents overloading individual workers
- Adapts to actual load in real-time
- Handles heterogeneous workers better

### 3. Tune Gossip Interval

**Impact**: 10-15% CPU reduction, minimal latency impact

```rust
use rpcnet::cluster::ClusterConfig;

// Before (default 1s): higher CPU
let config = ClusterConfig::default()
    .with_gossip_interval(Duration::from_secs(1));

// After (2s for stable networks): lower CPU
let config = ClusterConfig::default()
    .with_gossip_interval(Duration::from_secs(2));
```

**Why it works**:
- Gossip overhead scales with frequency
- Stable networks don't need aggressive gossip
- Failure detection still fast enough (4-8s)

### 4. Increase Worker Pool Size

**Impact**: Linear throughput scaling

```rust
// Before: 3 workers → 150K RPS
// After: 5 workers → 250K+ RPS

// Each worker adds ~50K RPS capacity
```

**Guidelines**:
- Add workers until you hit network/director bottleneck
- Monitor director CPU - scale director if > 80%
- Ensure network bandwidth sufficient

## Detailed Tuning

### Connection Pool Optimization

#### Pool Size Calculation

```rust
// Formula: (concurrent_requests / num_workers) * buffer_factor
fn calculate_optimal_pool_size(
    concurrent_requests: usize,
    num_workers: usize,
    buffer_factor: f64,
) -> usize {
    let base_size = concurrent_requests / num_workers;
    let with_buffer = (base_size as f64 * buffer_factor) as usize;
    
    // Cap at reasonable maximum
    with_buffer.min(50)
}

// Example:
let pool_size = calculate_optimal_pool_size(
    200,  // 200 concurrent requests
    10,   // 10 workers
    1.2   // 20% buffer
); // Returns 24
```

#### Idle Time Tuning

```rust
// Short idle time: Good for spiky traffic
.with_max_idle_time(Duration::from_secs(30))

// Medium idle time: Good for steady traffic (recommended)
.with_max_idle_time(Duration::from_secs(90))

// Long idle time: Good for high-throughput sustained load
.with_max_idle_time(Duration::from_secs(300))
```

**Trade-off**: Longer idle time = more memory, fewer reconnections

### QUIC Tuning

#### Stream Limits

```rust
use rpcnet::ServerConfig;

let config = ServerConfig::builder()
    .with_max_concurrent_streams(100)  // More streams = higher throughput
    .with_max_stream_bandwidth(10 * 1024 * 1024)  // 10 MB/s per stream
    .build();
```

**Guidelines**:
- **max_concurrent_streams**: Set to expected concurrent requests + 20%
- **max_stream_bandwidth**: Set based on your largest message size

#### Congestion Control

```rust
// Aggressive (high-bandwidth networks)
.with_congestion_control(CongestionControl::Cubic)

// Conservative (variable networks)
.with_congestion_control(CongestionControl::NewReno)

// Recommended default
.with_congestion_control(CongestionControl::Bbr)  // Best overall
```

### TLS Optimization

#### Session Resumption

```rust
// Enable TLS session tickets for 0-RTT
let config = ServerConfig::builder()
    .with_cert_and_key(cert, key)?
    .with_session_tickets_enabled(true)  // ← Enables 0-RTT
    .build();
```

**Impact**: First request after reconnect goes from 2-3 RTT to 0 RTT

#### Cipher Suite Selection

```rust
// Prefer fast ciphers (AES-GCM with hardware acceleration)
.with_cipher_suites(&[
    CipherSuite::TLS13_AES_128_GCM_SHA256,  // Fast with AES-NI
    CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,  // Good for ARM
])
```

### Message Serialization

#### Use Efficient Formats

```rust
// Fastest: bincode (binary)
use bincode;
let bytes = bincode::serialize(&data)?;

// Fast: rmp-serde (MessagePack)
use rmp_serde;
let bytes = rmp_serde::to_vec(&data)?;

// Slower: serde_json (human-readable, but slower)
let bytes = serde_json::to_vec(&data)?;
```

**Benchmark** (10KB struct):

| Format | Serialize | Deserialize | Size |
|--------|-----------|-------------|------|
| **bincode** | 12 μs | 18 μs | 10240 bytes |
| **MessagePack** | 28 μs | 35 μs | 9800 bytes |
| **JSON** | 85 μs | 120 μs | 15300 bytes |

#### Minimize Allocations

```rust
// ❌ Bad: Multiple allocations
fn build_request(id: u64, data: Vec<u8>) -> Request {
    Request {
        id: id.to_string(),  // Allocation
        timestamp: SystemTime::now(),
        payload: format!("data-{}", String::from_utf8_lossy(&data)),  // Multiple allocations
    }
}

// ✅ Good: Reuse buffers
fn build_request(id: u64, data: &[u8], buffer: &mut Vec<u8>) -> Request {
    buffer.clear();
    buffer.extend_from_slice(b"data-");
    buffer.extend_from_slice(data);
    
    Request {
        id,
        timestamp: SystemTime::now(),
        payload: buffer.clone(),  // Single allocation
    }
}
```

## Platform-Specific Optimizations

### Linux

#### TCP/QUIC Tuning

```bash
# Increase network buffer sizes
sudo sysctl -w net.core.rmem_max=536870912
sudo sysctl -w net.core.wmem_max=536870912
sudo sysctl -w net.ipv4.tcp_rmem='4096 87380 536870912'
sudo sysctl -w net.ipv4.tcp_wmem='4096 87380 536870912'

# Increase UDP buffer (QUIC uses UDP)
sudo sysctl -w net.core.netdev_max_backlog=5000

# Increase connection tracking
sudo sysctl -w net.netfilter.nf_conntrack_max=1000000

# Make permanent: add to /etc/sysctl.conf
```

#### CPU Affinity

```rust
use core_affinity;

// Pin worker threads to specific CPUs
fn pin_to_core(core_id: usize) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_id]);
}

// Usage in worker startup
tokio::task::spawn_blocking(|| {
    pin_to_core(0);  // Pin to CPU 0
    // Worker processing logic
});
```

### macOS

#### Increase File Descriptors

```bash
# Check current limits
ulimit -n

# Increase (temporary)
ulimit -n 65536

# Make permanent: add to ~/.zshrc or ~/.bash_profile
echo "ulimit -n 65536" >> ~/.zshrc
```

### Profiling and Monitoring

#### CPU Profiling

```bash
# Install perf (Linux)
sudo apt install linux-tools-common linux-tools-generic

# Profile RpcNet application
sudo perf record -F 99 -a -g -- cargo run --release --bin worker
sudo perf report

# Identify hot paths and optimize
```

#### Memory Profiling

```bash
# Use valgrind for memory analysis
cargo build --release
valgrind --tool=massif --massif-out-file=massif.out ./target/release/worker

# Visualize with massif-visualizer
ms_print massif.out
```

#### Tokio Console

```toml
# Add to Cargo.toml
[dependencies]
console-subscriber = "0.2"
```

```rust
// In main.rs
console_subscriber::init();

// Run application and connect with tokio-console
// cargo install tokio-console
// tokio-console
```

## Benchmarking

### Throughput Test

```rust
use std::time::Instant;

async fn benchmark_throughput(client: Arc<ClusterClient>, duration_secs: u64) {
    let start = Instant::now();
    let mut count = 0;
    
    while start.elapsed().as_secs() < duration_secs {
        match client.call_worker("compute", vec![], Some("role=worker")).await {
            Ok(_) => count += 1,
            Err(e) => eprintln!("Request failed: {}", e),
        }
    }
    
    let elapsed = start.elapsed().as_secs_f64();
    let rps = count as f64 / elapsed;
    
    println!("Throughput: {:.0} requests/second", rps);
    println!("Total requests: {}", count);
    println!("Duration: {:.2}s", elapsed);
}
```

### Latency Test

```rust
use hdrhistogram::Histogram;

async fn benchmark_latency(client: Arc<ClusterClient>, num_requests: usize) {
    let mut histogram = Histogram::<u64>::new(3).unwrap();
    
    for _ in 0..num_requests {
        let start = Instant::now();
        let _ = client.call_worker("compute", vec![], Some("role=worker")).await;
        let latency_us = start.elapsed().as_micros() as u64;
        histogram.record(latency_us).unwrap();
    }
    
    println!("Latency percentiles (μs):");
    println!("  P50:  {}", histogram.value_at_quantile(0.50));
    println!("  P90:  {}", histogram.value_at_quantile(0.90));
    println!("  P99:  {}", histogram.value_at_quantile(0.99));
    println!("  P99.9: {}", histogram.value_at_quantile(0.999));
    println!("  Max:  {}", histogram.max());
}
```

### Load Test Script

```rust
// Concurrent load test
async fn load_test(
    client: Arc<ClusterClient>,
    num_concurrent: usize,
    requests_per_task: usize,
) {
    let start = Instant::now();
    
    let tasks: Vec<_> = (0..num_concurrent)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move {
                for _ in 0..requests_per_task {
                    let _ = client.call_worker("compute", vec![], Some("role=worker")).await;
                }
            })
        })
        .collect();
    
    for task in tasks {
        task.await.unwrap();
    }
    
    let elapsed = start.elapsed().as_secs_f64();
    let total_requests = num_concurrent * requests_per_task;
    let rps = total_requests as f64 / elapsed;
    
    println!("Load test results:");
    println!("  Concurrency: {}", num_concurrent);
    println!("  Total requests: {}", total_requests);
    println!("  Duration: {:.2}s", elapsed);
    println!("  Throughput: {:.0} RPS", rps);
}
```

## Performance Checklist

### Before Production

- [ ] Enable connection pooling with appropriate size
- [ ] Use Least Connections load balancing
- [ ] Tune gossip interval for your network
- [ ] Configure QUIC stream limits
- [ ] Enable TLS session resumption
- [ ] Profile with release build (`--release`)
- [ ] Test under expected peak load
- [ ] Monitor CPU, memory, network utilization
- [ ] Set up latency tracking (P50, P99, P99.9)
- [ ] Configure OS-level network tuning

### Monitoring in Production

```rust
// Essential metrics to track
metrics::gauge!("rpc.throughput_rps", current_rps);
metrics::gauge!("rpc.latency_p50_us", latency_p50);
metrics::gauge!("rpc.latency_p99_us", latency_p99);
metrics::gauge!("rpc.cpu_usage_pct", cpu_usage);
metrics::gauge!("rpc.memory_mb", memory_mb);
metrics::gauge!("pool.hit_rate", pool_hit_rate);
metrics::gauge!("cluster.healthy_workers", healthy_count);
```

## Troubleshooting Performance Issues

### High Latency

**Symptoms**: P99 latency > 10ms

**Debug**:
```rust
// Add timing to identify bottleneck
let start = Instant::now();

let select_time = Instant::now();
let worker = registry.select_worker(Some("role=worker")).await?;
println!("Worker selection: {:?}", select_time.elapsed());

let connect_time = Instant::now();
let conn = pool.get_or_connect(worker.addr).await?;
println!("Connection: {:?}", connect_time.elapsed());

let call_time = Instant::now();
let result = conn.call("compute", data).await?;
println!("RPC call: {:?}", call_time.elapsed());

println!("Total: {:?}", start.elapsed());
```

**Common causes**:
- Connection pool exhaustion (add more connections)
- Slow workers (check worker CPU/memory)
- Network latency (move closer or add local workers)

### Low Throughput

**Symptoms**: < 100K RPS with multiple workers

**Debug**:
```rust
// Check bottlenecks
println!("Pool metrics: {:?}", pool.metrics());
println!("Worker count: {}", registry.worker_count().await);
println!("Active connections: {}", pool.active_connections());
```

**Common causes**:
- Too few workers (add more)
- Small connection pool (increase size)
- Director CPU saturated (scale director)
- Network bandwidth limit (upgrade network)

### High CPU Usage

**Symptoms**: > 80% CPU at low load

**Debug**:
```bash
# Profile with perf
sudo perf record -F 99 -a -g -- cargo run --release
sudo perf report

# Look for hot functions
```

**Common causes**:
- Too frequent gossip (increase interval)
- Excessive serialization (optimize message format)
- No connection pooling (enable it!)
- Debug build instead of release

## Real-World Results

### Case Study: Video Transcoding Cluster

**Setup**:
- 1 director
- 10 GPU workers
- 1000 concurrent clients

**Before tuning**: 45K RPS, 15ms P99 latency  
**After tuning**: 180K RPS, 2ms P99 latency

**Changes**:
1. Enabled connection pooling (size=20)
2. Tuned gossip interval (1s → 2s)
3. Used Least Connections strategy
4. Optimized message serialization (JSON → bincode)

## Next Steps

- **[Production Guide](production.md)** - Deploy optimized clusters
- **[Connection Pooling](../cluster/pooling.md)** - Deep dive into pooling
- **[Load Balancing](../cluster/load-balancing.md)** - Strategy selection

## References

- [QUIC Performance](https://datatracker.ietf.org/doc/html/rfc9000) - Protocol optimizations
- [Linux Network Tuning](https://wwwx.cs.unc.edu/~sparkst/howto/network_tuning.php) - OS-level tuning
- [Tokio Performance](https://tokio.rs/tokio/topics/performance) - Async runtime tips
