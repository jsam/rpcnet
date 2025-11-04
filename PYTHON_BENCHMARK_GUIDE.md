# Python Interop Benchmark Guide

## Overview

The `python_interop` benchmark measures the performance of Python client ↔ Rust server RPC calls, providing insights into:

- **MessagePack serialization overhead** compared to bincode
- **PyO3 bridge overhead** for cross-language calls
- **Real-world performance** of Python bindings in production scenarios

## Running the Benchmark

### Prerequisites

1. **Build Python bindings:**
   ```bash
   maturin develop --features python --release
   ```

2. **Ensure test certificates exist:**
   ```bash
   mkdir -p certs
   cd certs
   openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \
     -out test_cert.pem -days 365 -nodes -subj "/CN=localhost"
   cd ..
   ```

### Run Benchmarks

```bash
# Run Python interop benchmarks
cargo bench --bench python_interop --features python

# Run with performance features (jemalloc)
cargo bench --bench python_interop --features "python,perf"

# Compare with pure Rust benchmarks
cargo bench --bench simple           # Rust↔Rust baseline
cargo bench --bench python_interop --features python  # Python↔Rust
```

### View Results

```bash
# Open HTML report
open target/criterion/report/index.html

# View specific benchmark
open target/criterion/python_to_rust/report/index.html
open target/criterion/interop_comparison/report/index.html
```

## Benchmark Categories

### 1. Python → Rust (MessagePack)

**Benchmark**: `python_to_rust`

Measures end-to-end Python client calling Rust server:
- Python serialization (dict → MessagePack)
- Network transfer
- Rust deserialization (MessagePack → HashMap)
- Echo processing
- Rust serialization (HashMap → MessagePack)
- Python deserialization (MessagePack → dict)

**Payload Sizes**:
- 100 bytes - Small messages
- 1 KB - Typical requests
- 10 KB - Medium payloads
- 100 KB - Large payloads

### 2. Interop Comparison

**Benchmark**: `interop_comparison`

Direct comparison at 1KB payload:
- `rust_to_rust_bincode` - Baseline (Rust client + bincode)
- `python_to_rust_msgpack` - Python client + MessagePack

**Metrics**:
- Latency (μs)
- Throughput (requests/sec)
- Serialization overhead (%)

## Expected Results

Based on serialization benchmarks:

### Latency (1KB payload)

| Configuration | Expected Latency | Notes |
|--------------|------------------|-------|
| Rust↔Rust (bincode) | ~30 μs | Baseline (fastest) |
| Python↔Rust (MessagePack) | ~60-100 μs | +2-3x overhead |

**Overhead Sources**:
- MessagePack vs bincode: ~2x (28μs vs 12μs)
- PyO3 bridge: ~30-40μs
- Python runtime: ~10-20μs

### Throughput (1KB payload)

| Configuration | Expected RPS | Notes |
|--------------|--------------|-------|
| Rust↔Rust | ~30,000-50,000 | Single-threaded |
| Python↔Rust | ~10,000-15,000 | Python GIL limitations |

### Scalability

**Rust Server** (scales linearly):
- 1 worker: 50K RPS
- 4 workers: 200K RPS
- 8 workers: 400K RPS

**Python Client** (GIL bottleneck):
- 1 client: 10-15K RPS
- Multiple processes: Linear scaling
- Recommendation: Use multiprocessing for high load

## Performance Tips

### For Production Python Clients

1. **Connection Pooling**:
   ```python
   # Reuse connections
   client = await RpcClient.connect(...)
   # Make many calls with same client
   ```

2. **Batch Requests**:
   ```python
   # Send multiple requests concurrently
   tasks = [client.call("method", data) for data in batch]
   results = await asyncio.gather(*tasks)
   ```

3. **Multiprocessing**:
   ```python
   # Bypass GIL for high throughput
   from multiprocessing import Process

   def worker():
       asyncio.run(make_rpc_calls())

   processes = [Process(target=worker) for _ in range(4)]
   ```

4. **Payload Optimization**:
   ```python
   # Keep payloads small when possible
   # MessagePack is efficient but still has overhead
   request = {"id": 123}  # Small
   # vs
   request = {"data": large_blob}  # Slower
   ```

## Interpreting Results

### Good Performance

```
python_to_rust/1KB     time:   [80.23 μs 82.45 μs 84.67 μs]
                       thrpt:  [12,100 reqs/s]
```

✅ **Acceptable**: ~80μs latency, ~12K RPS for 1KB payload

### Poor Performance

```
python_to_rust/1KB     time:   [500.12 μs 520.45 μs 540.23 μs]
                       thrpt:  [1,900 reqs/s]
```

⚠️ **Issues**: >500μs latency suggests problems:
- Network issues
- Server overload
- Python bindings not built in release mode
- GC pressure

### Comparison Ratio

```
interop_comparison/rust_to_rust_bincode      30 μs
interop_comparison/python_to_rust_msgpack    90 μs
```

**Overhead Ratio**: 3x (expected and acceptable)

If ratio > 5x, investigate:
- Build Python bindings with `--release`
- Check server load
- Profile Python client code

## Troubleshooting

### Benchmark Skipped

```
⚠️  Skipping Python benchmarks: Python bindings not built
```

**Fix**:
```bash
maturin develop --features python --release
```

### Import Error

```
Python benchmark failed: ModuleNotFoundError: No module named '_rpcnet'
```

**Fix**:
```bash
# Ensure bindings are in target/release
ls target/release/_rpcnet*.so

# Rebuild if missing
maturin develop --features python --release
```

### Connection Refused

```
Python benchmark failed: ConnectionError
```

**Fix**:
- Ensure no other process using ports 19000-19101
- Check firewall settings
- Increase server startup delay in benchmark code

### Slow Performance

If benchmarks are significantly slower than expected:

1. **Check build mode**:
   ```bash
   cargo bench --features python --release
   ```

2. **Check CPU governor** (Linux):
   ```bash
   cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
   # Should be "performance", not "powersave"
   ```

3. **Check Python optimization**:
   ```bash
   python3 -c "import sys; print(sys.flags.optimize)"
   # Use: python3 -O for optimized mode
   ```

## Integration with CI

### GitHub Actions

```yaml
- name: Build Python bindings
  run: |
    pip install maturin
    maturin develop --features python --release

- name: Run Python interop benchmarks
  run: |
    cargo bench --bench python_interop --features python -- --output-format bencher

- name: Upload benchmark results
  uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: 'cargo'
    output-file-path: target/criterion/python_interop/base/estimates.json
```

## Contribution Guidelines

When modifying Python bindings or serialization:

1. **Run benchmarks before and after**:
   ```bash
   cargo bench --bench python_interop --features python -- --save-baseline before
   # Make changes
   cargo bench --bench python_interop --features python -- --baseline before
   ```

2. **Check for regressions**:
   - Latency increase > 10%: Investigate
   - Throughput decrease > 10%: Investigate
   - Both: Likely a real regression

3. **Document performance changes** in PR:
   ```
   Performance Impact:
   - 1KB latency: 82μs → 78μs (-5%)
   - Throughput: 12K → 13K RPS (+8%)
   ```

## References

- [MessagePack Benchmark](../docs/mdbook/src/advanced/performance.md)
- [Python Bindings Guide](../examples/python/cluster/README.md)
- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)

## FAQ

**Q: Why is Python slower than Rust?**

A: Multiple factors:
- MessagePack vs bincode serialization (~2x)
- PyO3 bridge overhead (~30-40μs)
- Python GIL and runtime (~10-20μs)
- This is expected and acceptable for cross-language RPC

**Q: When should I use Python clients?**

A: Python clients are ideal for:
- ✅ Tools and scripts
- ✅ Data processing pipelines
- ✅ Admin interfaces
- ✅ Moderate throughput (<50K RPS per process)
- ❌ Ultra-low latency requirements (<10μs)
- ❌ Maximum throughput (>100K RPS single process)

**Q: Can I improve Python performance?**

A: Yes, several options:
1. Use connection pooling and reuse
2. Batch requests when possible
3. Use multiprocessing to bypass GIL
4. Keep payloads small
5. Profile with `py-spy` or `cProfile`

**Q: Should I optimize for Python or Rust?**

A: **Optimize the server (Rust)**. The server handles many clients, so server optimizations have multiplicative benefits. Python client optimization is secondary.
