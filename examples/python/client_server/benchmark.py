#!/usr/bin/env python3
import sys
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(Path(__file__).parent / "generated"))

from benchmarkservice.client import BenchmarkServiceBlockingClient
from benchmarkservice.types import NoopRequest

def worker_thread(thread_id: int, requests_per_thread: int):
    client = BenchmarkServiceBlockingClient.connect(
        addr="127.0.0.1:50051",
        cert_path="../../../certs/test_cert.pem",
        server_name="localhost",
        timeout_secs=10
    )
    
    request = NoopRequest()
    start = time.perf_counter()
    
    for i in range(requests_per_thread):
        client.noop(request)
    
    elapsed = time.perf_counter() - start
    return thread_id, requests_per_thread, elapsed

def main():
    print("=" * 70)
    print("🚀 RpcNet Concurrent Benchmark")
    print("=" * 70)
    
    num_threads = 10
    requests_per_thread = 1000
    total_requests = num_threads * requests_per_thread
    
    print(f"Threads: {num_threads}")
    print(f"Requests/thread: {requests_per_thread}")
    print(f"Total: {total_requests}")
    print("=" * 70)
    
    print("\n🔥 Running...")
    overall_start = time.perf_counter()
    
    with ThreadPoolExecutor(max_workers=num_threads) as executor:
        futures = [
            executor.submit(worker_thread, i, requests_per_thread) 
            for i in range(num_threads)
        ]
        
        completed = 0
        for future in as_completed(futures):
            thread_id, count, elapsed = future.result()
            completed += count
            print(f"  Thread {thread_id}: {count} req in {elapsed:.2f}s ({count/elapsed:.0f} req/s)")
    
    overall_time = time.perf_counter() - overall_start
    
    print("\n" + "=" * 70)
    print("📊 RESULTS")
    print("=" * 70)
    print(f"Total Requests: {total_requests}")
    print(f"Total Time: {overall_time:.2f}s")
    print(f"Throughput: {total_requests / overall_time:.0f} req/s")
    print(f"Avg Latency: {(overall_time / total_requests) * 1000:.3f} ms")
    print("=" * 70)

if __name__ == "__main__":
    main()
