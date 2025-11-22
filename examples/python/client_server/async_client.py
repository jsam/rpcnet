#!/usr/bin/env python3
import asyncio
import sys
import time
import os
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "generated"))

from benchmarkservice.client import BenchmarkServiceClient
from benchmarkservice.types import NoopRequest, BenchmarkRequest

async def main():
    print("=" * 70)
    print("🚀 RpcNet Async Client")
    print("=" * 70)

    # Use environment variable or default relative path
    cert_path = os.getenv("CERT_PATH", str(Path(__file__).parent.parent.parent.parent / "certs" / "test_cert.pem"))
    
    client = await BenchmarkServiceClient.connect(
        addr="127.0.0.1:50051",
        cert_path=cert_path,
        server_name="localhost"
    )
    print("✅ Connected")

    print("\n🔥 Warmup (100 requests)...")
    request = NoopRequest()
    for _ in range(100):
        await client.noop(request)
    print("✅ Warmup complete")

    print("\n📊 Benchmark (10000 requests)...")
    iterations = 10000
    start = time.perf_counter()
    
    for i in range(iterations):
        await client.noop(request)
        if (i + 1) % 1000 == 0:
            print(f"  {i + 1}/{iterations}")
    
    total_time = time.perf_counter() - start
    
    print("\n" + "=" * 70)
    print("📊 RESULTS")
    print("=" * 70)
    print(f"Total Requests: {iterations}")
    print(f"Total Time: {total_time:.2f}s")
    print(f"Throughput: {iterations / total_time:.1f} req/s")
    print(f"Avg Latency: {(total_time / iterations) * 1000:.3f} ms")
    print("=" * 70)

    print("\n📊 Maximum throughput test (10000 requests, fully concurrent)...")
    iterations = 10000
    start = time.perf_counter()
    
    # Launch all requests concurrently without waiting
    tasks = [client.noop(request) for _ in range(iterations)]
    await asyncio.gather(*tasks)
    
    total_time = time.perf_counter() - start
    print(f"✅ Max throughput: {iterations / total_time:.0f} req/s")
    print(f"   Avg latency: {(total_time / iterations) * 1000:.3f} ms")

    print("\n📊 Testing process method (1000 requests)...")
    proc_request = BenchmarkRequest(message="test", value=42)
    start = time.perf_counter()
    
    for i in range(1000):
        response = await client.process(proc_request)
        assert response.echo == "test"
        assert response.doubled == 84
    
    proc_time = time.perf_counter() - start
    print(f"✅ Process: {1000 / proc_time:.1f} req/s")

if __name__ == "__main__":
    asyncio.run(main())
