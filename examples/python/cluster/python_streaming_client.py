#!/usr/bin/env python3
"""
Python streaming client for RpcNet cluster example.

This demonstrates the complete end-to-end flow:
1. Connect to director registry to get an available worker
2. Connect to the worker
3. Send compute tasks to the worker
4. Handle responses

Prerequisites:
1. Run the Rust cluster (director + workers)
2. Build Python bindings: maturin develop --features python --release
3. Generate Python code for both services
"""

import asyncio
import sys
import os
import time

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

from directorregistry import DirectorRegistryClient, GetWorkerRequest, DirectorError
from inference import InferenceClient, InferenceRequest, InferenceError


async def main():
    print("=" * 70)
    print("Python Streaming Client - Full Workflow Demo")
    print("=" * 70)
    print()
    print("This demonstrates:")
    print("  1. Python → Rust Director (Registry service)")
    print("  2. Python → Rust Worker (Compute service)")
    print("  3. End-to-end task processing")
    print()

    # Configuration
    DIRECTOR_ADDR = os.getenv("DIRECTOR_ADDR", "127.0.0.1:61000")
    CERT_PATH = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")
    KEY_PATH = os.getenv("KEY_PATH", "../../../certs/test_key.pem")

    # Resolve cert and key paths relative to this file
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cert_path = os.path.join(script_dir, CERT_PATH)
    key_path = os.path.join(script_dir, KEY_PATH)

    if not os.path.exists(cert_path):
        print(f"❌ Certificate not found: {cert_path}")
        print(f"   Generate with:")
        print(f"   mkdir -p certs && cd certs")
        print(f"   openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \\")
        print(f"     -out test_cert.pem -days 365 -nodes -subj '/CN=localhost'")
        return 1

    print(f"📁 Using certificate: {cert_path}")
    print(f"🎯 Director address: {DIRECTOR_ADDR}")
    print()

    try:
        # ===================================================================
        # STEP 1: Connect to Director Registry
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 1: Connecting to Director Registry                        │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        director = await DirectorRegistryClient.connect(
            DIRECTOR_ADDR,
            cert_path=cert_path,
            key_path=key_path,
            server_name="localhost",
            timeout_secs=5,
        )
        print(f"✅ Connected to director at {DIRECTOR_ADDR}")
        print()

        # ===================================================================
        # STEP 2: Get Available Worker
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 2: Getting Available Worker                               │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        try:
            worker_info = await director.get_worker(
                GetWorkerRequest(
                    connection_id=None,
                    prompt="Streaming demo from Python"
                )
            )

            if not worker_info.success or not worker_info.worker_addr:
                print(f"❌ No workers available: {worker_info.message}")
                print()
                print("💡 Start a worker with:")
                print("   WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \\")
                print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
                print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
                return 1

            print(f"✅ Got worker assignment:")
            print(f"   Worker:  {worker_info.worker_label}")
            print(f"   Address: {worker_info.worker_addr}")
            print(f"   Connection ID: {worker_info.connection_id}")
            print()

        except Exception as e:
            if "NoWorkersAvailable" in str(e) or "NOWORKERSAVAILABLE" in str(e):
                print("❌ No workers available")
                print()
                print("💡 Start a worker with:")
                print("   WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \\")
                print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
                print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
                return 1
            else:
                raise

        # ===================================================================
        # STEP 3: Connect to Worker
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 3: Connecting to Worker                                   │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        worker = await InferenceClient.connect(
            worker_info.worker_addr,
            cert_path=cert_path,
            key_path=key_path,
            server_name="localhost",
            timeout_secs=30,
        )
        print(f"✅ Connected to worker at {worker_info.worker_addr}")
        print()

        # ===================================================================
        # STEP 4: Send Inference Requests
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 4: Sending Inference Requests                             │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        prompts = [
            "Hello, how are you?",
            "What is the meaning of life?",
            "Tell me a joke.",
            "Explain quantum computing",
            "Write a haiku about coding",
        ]

        print(f"📤 Sending {len(prompts)} inference requests to worker...")
        print()

        for i, prompt in enumerate(prompts, 1):
            start_time = time.time()

            try:
                response = await worker.infer(
                    InferenceRequest(
                        connection_id=worker_info.connection_id,
                        prompt=prompt
                    )
                )

                elapsed = (time.time() - start_time) * 1000

                print(f"Request {i}/{len(prompts)}:")
                print(f"  ✅ Success ({elapsed:.1f}ms)")
                print(f"  📝 Prompt:   {prompt}")
                print(f"  📊 Response: {response.text}")
                print()

            except Exception as e:
                elapsed = (time.time() - start_time) * 1000
                print(f"Request {i}/{len(prompts)}:")
                print(f"  ❌ Failed ({elapsed:.1f}ms)")
                print(f"  ⚠️  Error: {e}")
                print()

        # ===================================================================
        # STEP 5: Test Load Balancing (get multiple workers)
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 5: Testing Load Balancing                                 │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        print("📊 Requesting workers multiple times to test load balancing...")
        print()

        worker_counts = {}
        num_requests = 10

        for i in range(num_requests):
            try:
                info = await director.get_worker(
                    GetWorkerRequest(
                        connection_id=None,
                        prompt=f"Load balance test {i+1}"
                    )
                )

                if info.success and info.worker_label:
                    worker_label = info.worker_label
                    worker_counts[worker_label] = worker_counts.get(worker_label, 0) + 1
                    print(f"  Request {i+1:2d}: {worker_label:<15} (total: {worker_counts[worker_label]})")
                else:
                    print(f"  Request {i+1:2d}: ⚠️  {info.message}")

            except Exception as e:
                print(f"  Request {i+1:2d}: ❌ {e}")

        print()
        print("📈 Load Distribution:")
        for worker_label, count in sorted(worker_counts.items()):
            percentage = (count / num_requests) * 100
            bar = "█" * int(percentage / 5)
            print(f"  {worker_label:<15} {bar} {count:2d} ({percentage:5.1f}%)")

        # ===================================================================
        # Summary
        # ===================================================================
        print()
        print("=" * 70)
        print("✅ Python Streaming Client Demo Completed Successfully!")
        print("=" * 70)
        print()
        print("What was demonstrated:")
        print("  ✅ Python → Rust director (DirectorRegistry.get_worker)")
        print("  ✅ Python → Rust worker (Inference.infer)")
        print("  ✅ End-to-end inference processing")
        print("  ✅ Load balancing across workers")
        print("  ✅ Type-safe Python bindings")
        print("  ✅ MessagePack serialization (Python ↔ Rust)")
        print("  ✅ QUIC+TLS transport")
        print()
        print("Generated bindings used:")
        print("  • generated/directorregistry/ (DirectorRegistryClient)")
        print("  • generated/inference/ (InferenceClient)")
        print()
        print("Services:")
        print(f"  • Director: {DIRECTOR_ADDR}")
        print(f"  • Worker:   {worker_info.worker_addr}")
        print("=" * 70)

        return 0

    except ConnectionError as e:
        print()
        print(f"❌ Connection error: {e}")
        print()
        print("💡 Make sure the Rust cluster is running:")
        print()
        print("   # Terminal 1 - Director")
        print("   DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
        print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin director")
        print()
        print("   # Terminal 2 - Worker")
        print("   WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \\")
        print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
        print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
        return 1
    except Exception as e:
        print()
        print(f"❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
