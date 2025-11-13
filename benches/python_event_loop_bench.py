#!/usr/bin/env python3
"""
Benchmark for Python Event Loop Executor

This benchmarks the new persistent event loop thread implementation.
"""
import asyncio
import sys
import time
sys.path.insert(0, '.venv/lib/python3.13/site-packages')

import _rpcnet


async def benchmark_handler_invocations(num_calls=1000):
    """Benchmark handler invocation latency and throughput"""
    print(f"\n{'='*60}")
    print(f"Benchmarking {num_calls} handler invocations...")
    print(f"{'='*60}")

    # Create server
    server_config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="127.0.0.1:18888",
        key_path="certs/test_key.pem",
        server_name="localhost",
        timeout_secs=30
    )
    server = _rpcnet.RpcServer(server_config)

    # Define async handler
    call_count = [0]
    async def bench_handler(request_bytes: bytes) -> bytes:
        call_count[0] += 1
        return request_bytes

    # Register handler
    await server.register("bench", bench_handler)

    # Start server in background
    async def run_server():
        try:
            await server.serve()
        except asyncio.CancelledError:
            pass

    server_task = asyncio.create_task(run_server())
    await asyncio.sleep(1.0)  # Give server time to start

    try:
        # Create client
        client_config = _rpcnet.RpcConfig(
            cert_path="certs/test_cert.pem",
            bind_addr="0.0.0.0:0",
            server_name="localhost",
            timeout_secs=30
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:18888", client_config)

        # Warmup
        print("Warming up...")
        for _ in range(10):
            await client.call("bench", b"warmup")

        # Benchmark - Sequential calls
        print(f"\nSequential calls...")
        test_data = b"x" * 100
        start_time = time.perf_counter()

        for i in range(num_calls):
            await client.call("bench", test_data)
            if (i + 1) % 100 == 0:
                print(f"  {i + 1}/{num_calls} calls completed...")

        end_time = time.perf_counter()
        duration = end_time - start_time

        # Results
        print(f"\n{'='*60}")
        print(f"RESULTS:")
        print(f"{'='*60}")
        print(f"Total calls:        {num_calls}")
        print(f"Total time:         {duration:.3f} seconds")
        print(f"Avg latency:        {(duration / num_calls) * 1000:.2f} ms/call")
        print(f"Throughput:         {num_calls / duration:.2f} calls/sec")
        print(f"Handler invocations: {call_count[0]}")
        print(f"{'='*60}\n")

        # Test different payload sizes
        print("\nLatency by payload size:")
        print(f"{'='*60}")
        for size in [10, 100, 1024, 10240]:
            payload = b"x" * size
            start = time.perf_counter()
            for _ in range(100):
                await client.call("bench", payload)
            elapsed = time.perf_counter() - start
            avg_latency = (elapsed / 100) * 1000
            print(f"  {size:6d} bytes: {avg_latency:6.2f} ms/call")
        print(f"{'='*60}\n")

        return True

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


async def main():
    print("\n" + "="*60)
    print("Python Event Loop Executor Benchmark")
    print("="*60)
    print("\nThis benchmarks the NEW persistent event loop thread:")
    print("- Single dedicated thread with reused asyncio event loop")
    print("- Channel-based request/response communication")
    print("- GIL released while waiting for requests")
    print()

    await benchmark_handler_invocations(num_calls=500)

    print("\n✅ Benchmark complete!")


if __name__ == "__main__":
    result = asyncio.run(main())
    sys.exit(0 if result else 1)
