#!/usr/bin/env python3
"""
Realistic Python→Rust RPC Benchmark

This benchmark uses a long-lived Python client connection to measure
real-world performance without subprocess overhead.

The benchmark automatically starts and stops a Rust server, making it
completely self-contained.

Run with: .venv/bin/python benches/python_realistic_bench.py
"""

import asyncio
import time
import sys
import statistics
import subprocess
import signal
import os

# Add venv to path if needed
sys.path.insert(0, '.venv/lib/python3.13/site-packages')

import _rpcnet


def start_server(port: int = 8080):
    """
    Start the Rust RPC server in the background.

    Returns:
        subprocess.Popen: The server process
    """
    print(f"🚀 Starting Rust server on port {port}...")

    # Start server as subprocess
    # Redirect stdout/stderr to suppress server logs
    server_process = subprocess.Popen(
        ["cargo", "run", "--example", "basic_server", "--quiet"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid if hasattr(os, 'setsid') else None  # Unix only
    )

    # Give server time to start
    time.sleep(2)

    # Check if server started successfully
    if server_process.poll() is not None:
        raise RuntimeError(f"Server failed to start (exit code: {server_process.returncode})")

    print(f"✅ Server started (PID: {server_process.pid})\n")
    return server_process


def stop_server(server_process):
    """
    Stop the Rust RPC server gracefully.

    Args:
        server_process: The server process returned by start_server()
    """
    if server_process and server_process.poll() is None:
        print(f"\n🛑 Stopping server (PID: {server_process.pid})...")

        try:
            # Try graceful shutdown first
            if hasattr(os, 'killpg'):
                # Unix: kill the process group
                os.killpg(os.getpgid(server_process.pid), signal.SIGTERM)
            else:
                # Windows fallback
                server_process.terminate()

            # Wait for graceful shutdown (max 3 seconds)
            try:
                server_process.wait(timeout=3)
                print("✅ Server stopped gracefully")
            except subprocess.TimeoutExpired:
                # Force kill if it doesn't stop
                if hasattr(os, 'killpg'):
                    os.killpg(os.getpgid(server_process.pid), signal.SIGKILL)
                else:
                    server_process.kill()
                server_process.wait()
                print("⚠️  Server force-killed")
        except Exception as e:
            print(f"⚠️  Error stopping server: {e}")


async def bench_python_to_rust(server_addr: str, num_iterations: int = 1000, payload_size: int = 1024):
    """
    Benchmark Python→Rust RPC calls with a persistent connection.

    Args:
        server_addr: Server address (e.g., "127.0.0.1:19000")
        num_iterations: Number of RPC calls to make
        payload_size: Size of payload in bytes
    """
    # Create config
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="0.0.0.0:0",
        server_name="localhost",
        timeout_secs=30
    )

    # Connect once (reuse connection)
    print(f"🔌 Connecting to {server_addr}...")
    client = await _rpcnet.RpcClient.connect(server_addr, config)
    print(f"✅ Connected successfully\n")

    # Create payload
    payload = {"data": list(b"x" * payload_size)}
    serialized = _rpcnet.python_to_msgpack_py(payload)
    print(f"📦 Payload size: {len(serialized)} bytes ({payload_size} byte data)")

    # Warmup
    print(f"🔥 Warming up (10 calls)...")
    for _ in range(10):
        await client.call("echo", serialized)

    # Benchmark
    print(f"⏱️  Running benchmark ({num_iterations} iterations)...\n")
    latencies = []

    start_total = time.perf_counter()
    for i in range(num_iterations):
        start = time.perf_counter()
        response = await client.call("echo", serialized)
        end = time.perf_counter()

        latency_us = (end - start) * 1_000_000
        latencies.append(latency_us)

        if (i + 1) % 100 == 0:
            print(f"  Progress: {i+1}/{num_iterations} calls")

    end_total = time.perf_counter()
    total_time = end_total - start_total

    # Calculate statistics
    avg_latency = statistics.mean(latencies)
    median_latency = statistics.median(latencies)
    p95_latency = statistics.quantiles(latencies, n=20)[18]  # 95th percentile
    p99_latency = statistics.quantiles(latencies, n=100)[98]  # 99th percentile
    min_latency = min(latencies)
    max_latency = max(latencies)
    std_dev = statistics.stdev(latencies)

    throughput = num_iterations / total_time

    # Print results
    print(f"\n{'='*60}")
    print(f"🎯 BENCHMARK RESULTS")
    print(f"{'='*60}")
    print(f"Total time:        {total_time:.2f} seconds")
    print(f"Iterations:        {num_iterations}")
    print(f"Throughput:        {throughput:.2f} requests/sec")
    print(f"\n📊 Latency (microseconds):")
    print(f"  Mean:            {avg_latency:.2f} µs")
    print(f"  Median:          {median_latency:.2f} µs")
    print(f"  Std Dev:         {std_dev:.2f} µs")
    print(f"  Min:             {min_latency:.2f} µs")
    print(f"  Max:             {max_latency:.2f} µs")
    print(f"  P95:             {p95_latency:.2f} µs")
    print(f"  P99:             {p99_latency:.2f} µs")
    print(f"{'='*60}\n")

    return {
        "total_time": total_time,
        "throughput": throughput,
        "avg_latency_us": avg_latency,
        "median_latency_us": median_latency,
        "p95_latency_us": p95_latency,
        "p99_latency_us": p99_latency,
    }


async def main():
    """Run realistic benchmarks for different payload sizes."""
    server_addr = "127.0.0.1:8080"
    iterations = 1000
    server_process = None

    print("""
╔════════════════════════════════════════════════════════════╗
║  RpcNet: Realistic Python→Rust RPC Benchmark              ║
║  (Long-lived connection, no subprocess overhead)           ║
║  (Automatically starts/stops server)                       ║
╚════════════════════════════════════════════════════════════╝
    """)

    try:
        # Start the server
        server_process = start_server(port=8080)

        # Test different payload sizes
        payload_sizes = [100, 1024, 10_240]

        for size in payload_sizes:
            print(f"{'─'*60}")
            print(f"Testing with {size} byte payload:")
            print(f"{'─'*60}")
            await bench_python_to_rust(server_addr, iterations, size)
            await asyncio.sleep(1)  # Brief pause between benchmarks

    except ConnectionRefusedError:
        print("\n❌ ERROR: Could not connect to server")
        print("   Server may have failed to start properly")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ ERROR: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
    finally:
        # Always stop the server, even if benchmark fails
        stop_server(server_process)


if __name__ == "__main__":
    asyncio.run(main())
