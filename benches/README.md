# RpcNet Benchmarks

This directory contains performance benchmarks for RpcNet.

## Rust Benchmarks (Criterion)

### `simple.rs`
Basic RPC performance benchmarks:
- Unary calls with various payload sizes
- Concurrent request handling
- Serialization/deserialization overhead

**Run:**
```bash
cargo bench --bench simple
```

### `streaming.rs`
Streaming RPC performance:
- Server streaming (1→N)
- Client streaming (N→1)
- Bidirectional streaming (N→M)

**Run:**
```bash
cargo bench --bench streaming
```

### `python_interop.rs`
Python↔Rust interoperability benchmarks:
- **Subprocess-based benchmarks** (historical, high overhead)
  - `python_to_rust/*` - Spawns Python subprocess per benchmark iteration
  - `interop_comparison/*` - Compares Rust→Rust vs Python→Rust
  - ⚠️  **Not representative of real-world usage** due to subprocess spawning overhead

**Run:**
```bash
cargo bench --bench python_interop --features python
```

**Note:** These benchmarks show ~800x slower performance than Rust→Rust, but this is **artificial** due to subprocess overhead. For realistic Python performance, use `python_realistic_bench.py` instead.

## Python Benchmarks (Direct)

### `python_realistic_bench.py` ⭐ **Recommended**
Realistic Python→Rust RPC performance measurement with long-lived connections.

**Features:**
- **Fully self-contained**: Automatically starts and stops the Rust server
- Uses persistent Python client connection (no subprocess overhead)
- Tests multiple payload sizes (100B, 1KB, 10KB)
- Provides detailed statistics (mean, median, P95, P99)
- Measures throughput and latency distribution
- Graceful server shutdown after benchmarking

**Run:**
```bash
# Just run it - no manual server setup needed!
.venv/bin/python benches/python_realistic_bench.py
```

**Expected Results:**
```
Payload: 1KB
Total time:        X.XX seconds
Throughput:        XXX requests/sec
Latency:
  Mean:            XXX µs
  Median:          XXX µs
  P95:             XXX µs
  P99:             XXX µs
```

## Benchmark Comparison

| Benchmark Type | Overhead | Use Case | Representative? |
|---------------|----------|----------|-----------------|
| Rust→Rust (criterion) | Minimal | Pure Rust performance | ✅ Yes |
| Python→Rust (subprocess) | Very High | N/A | ❌ No |
| Python→Rust (realistic) | Realistic | Real Python clients | ✅ Yes |

## Tips

1. **For Python performance**: Always use `python_realistic_bench.py`
2. **For Rust performance**: Use `simple.rs` and `streaming.rs`
3. **For comparison**: Compare `rust_to_rust_msgpack` vs `python_realistic_bench.py` results

## MessagePack Migration

All benchmarks use MessagePack serialization (via `rmp-serde`) for both Rust and Python clients. This ensures:
- Consistent serialization across languages
- Fair performance comparisons
- Real-world representative results
