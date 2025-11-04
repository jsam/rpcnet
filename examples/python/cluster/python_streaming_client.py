#!/usr/bin/env python3
"""
Python streaming client for RpcNet cluster example.

This demonstrates how to use the generated Python bindings for streaming RPC.
It connects directly to a worker and uses the streaming generate() method.

Prerequisites:
1. Run the Rust cluster (director + worker) first
2. Build Python bindings: maturin develop --features python
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

from directorregistry import DirectorRegistryClient, GetWorkerRequest
from inference import InferenceClient, InferenceRequest, InferenceResponse


async def generate_requests():
    """Async generator that yields inference requests"""
    prompts = [
        "Hello, how are you?",
        "What is the meaning of life?",
        "Tell me a joke.",
    ]

    for i, prompt in enumerate(prompts):
        print(f"   📤 Sending request {i+1}: {prompt}")
        yield InferenceRequest(
            connection_id="python-streaming-client",
            prompt=prompt,
        )
        await asyncio.sleep(0.1)  # Small delay between requests


async def main():
    print("=" * 70)
    print("Python Streaming Client for RpcNet Cluster - Inference Demo")
    print("=" * 70)
    print()
    print("This demonstrates bidirectional streaming RPC:")
    print("  • Client sends multiple requests as a stream")
    print("  • Server generates responses as a stream")
    print("  • All using Python async generators!")
    print()

    # Configuration
    DIRECTOR_ADDR = os.getenv("DIRECTOR_ADDR", "127.0.0.1:61000")
    CERT_PATH = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")

    # Resolve cert path relative to this file
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cert_path = os.path.join(script_dir, CERT_PATH)

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
        # Step 1: Connect to director to get a worker
        print("1️⃣  Connecting to director to get a worker...")
        director = await DirectorRegistryClient.connect(
            DIRECTOR_ADDR,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=5,
        )
        print(f"   ✅ Connected to director")

        # Get a worker
        worker_info = await director.get_worker(
            GetWorkerRequest(
                connection_id=None,
                prompt="Streaming demo request"
            )
        )

        if not worker_info.success or not worker_info.worker_addr:
            print(f"   ❌ No workers available: {worker_info.message}")
            print()
            print("   💡 Start a worker with:")
            print("      WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 \\")
            print("        cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
            return 1

        worker_addr = worker_info.worker_addr
        print(f"   ✅ Got worker: {worker_info.worker_label} at {worker_addr}")
        print()

        # Step 2: Connect directly to the worker
        print("2️⃣  Connecting to worker for streaming RPC...")
        inference_client = await InferenceClient.connect(
            worker_addr,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=30,
        )
        print(f"   ✅ Connected to worker at {worker_addr}")
        print()

        # Step 3: Call streaming generate() method
        print("3️⃣  Calling streaming generate() method...")
        print()

        response_count = 0
        async for response in inference_client.generate(generate_requests()):
            response_count += 1
            print(f"   📥 Response {response_count}: {response}")
            print()

        print("=" * 70)
        print("✅ Streaming RPC completed successfully!")
        print()
        print("What was demonstrated:")
        print("  • Python async generator used for request stream")
        print("  • Python async iterator used for response stream")
        print("  • Bidirectional streaming over QUIC+TLS")
        print("  • Method name: 'Inference.generate'")
        print("  • Serialization: MessagePack (Python ↔ Rust)")
        print(f"  • Total requests sent: 3")
        print(f"  • Total responses received: {response_count}")
        print()
        print("Generated files:")
        print("  • examples/python/cluster/generated/inference/")
        print("    - types.py      (InferenceRequest, InferenceResponse, InferenceError)")
        print("    - client.py     (InferenceClient with streaming support)")
        print("    - server.py     (InferenceServer)")
        print("=" * 70)
        return 0

    except ConnectionError as e:
        print()
        print(f"❌ Connection error: {e}")
        print()
        print("💡 Make sure the Rust cluster is running:")
        print("   Terminal 1 - Director:")
        print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
        print("       cargo run --manifest-path examples/cluster/Cargo.toml --bin director")
        print()
        print("   Terminal 2 - Worker:")
        print("     WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 \\")
        print("       RUST_LOG=info cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
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
