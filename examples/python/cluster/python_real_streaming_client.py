#!/usr/bin/env python3
"""
Python streaming client demonstrating REAL bidirectional streaming with RpcNet.

This example uses the actual streaming `generate()` method which:
- Accepts an AsyncIterable of requests (client-side streaming)
- Returns an AsyncIterator of responses (server-side streaming)
- Demonstrates true bidirectional streaming over QUIC+TLS

Prerequisites:
1. Run the Rust cluster (director + worker)
2. Build Python bindings: maturin develop --features python --release
3. Generate Python code for both services
"""

import asyncio
import sys
import os
import time
from typing import AsyncIterator

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

from directorregistry import DirectorRegistryClient, GetWorkerRequest, DirectorError
from inference import InferenceClient, InferenceRequest


async def request_generator(connection_id: str, prompts: list[str]) -> AsyncIterator[InferenceRequest]:
    """
    Generate streaming requests.

    This is the client-side streaming part: we yield multiple requests
    that will be sent to the server as a stream.
    """
    for i, prompt in enumerate(prompts):
        print(f"   📤 Sending request {i+1}/{len(prompts)}: {prompt[:50]}...")
        yield InferenceRequest(
            connection_id=connection_id,
            prompt=prompt
        )
        # Small delay between requests to simulate realistic streaming
        await asyncio.sleep(0.1)


async def main():
    print("=" * 70)
    print("Python REAL Streaming Client - Bidirectional Streaming Demo")
    print("=" * 70)
    print()
    print("This demonstrates TRUE streaming RPC with:")
    print("  • Client-side streaming (AsyncIterable[InferenceRequest])")
    print("  • Server-side streaming (AsyncIterator[InferenceResponse])")
    print("  • Bidirectional: send multiple requests, receive multiple responses")
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
        print(f"   openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \\\\")
        print(f"     -out test_cert.pem -days 365 -nodes -subj '/CN=localhost'")
        return 1

    print(f"📁 Using certificate: {cert_path}")
    print(f"🎯 Director address: {DIRECTOR_ADDR}")
    print()

    try:
        # ===================================================================
        # STEP 1: Connect to Director and Get Worker
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 1: Getting Worker Assignment from Director                │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        director = await DirectorRegistryClient.connect(
            DIRECTOR_ADDR,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=5,
        )
        print(f"✅ Connected to director at {DIRECTOR_ADDR}")

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
            print("   WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \\\\")
            print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\\\")
            print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
            return 1

        print(f"✅ Got worker assignment:")
        print(f"   Worker:        {worker_info.worker_label}")
        print(f"   Address:       {worker_info.worker_addr}")
        print(f"   Connection ID: {worker_info.connection_id}")
        print()

        # ===================================================================
        # STEP 2: Connect to Worker
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 2: Connecting to Worker                                   │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        worker = await InferenceClient.connect(
            worker_info.worker_addr,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=60,  # Longer timeout for streaming
        )
        print(f"✅ Connected to worker at {worker_info.worker_addr}")
        print()

        # ===================================================================
        # STEP 3: Streaming Inference - Bidirectional
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ STEP 3: Bidirectional Streaming Inference                      │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()

        # Prepare multiple prompts to stream
        prompts = [
            "What is the capital of France?",
            "Explain quantum computing in simple terms",
            "Write a haiku about programming",
            "What are the benefits of Rust?",
            "Describe the QUIC protocol",
        ]

        print(f"📊 Streaming {len(prompts)} requests to worker...")
        print()

        # Track statistics
        response_count = 0
        token_count = 0
        error_count = 0
        start_time = time.time()
        connected = False

        try:
            # THIS IS THE KEY: True bidirectional streaming using generated client
            # - request_generator() yields requests (client → server stream)
            # - worker.generate() returns async iterator (server → client stream)
            # - Generated client automatically handles enum deserialization!

            response_stream = worker.generate(request_generator(worker_info.connection_id, prompts))

            async for response in response_stream:
                response_count += 1

                print(f"📥 Response {response_count}:")
                print(f"   Type: {type(response).__name__}")

                # Handle different InferenceResponse variants using generated dataclasses
                from inference.types import (
                    InferenceResponseConnected,
                    InferenceResponseToken,
                    InferenceResponseError,
                    InferenceResponseDone
                )

                if isinstance(response, InferenceResponseConnected):
                    print(f"   🔗 Connected to worker: {response.worker}")
                    print(f"   🆔 Connection ID: {response.connection_id}")
                    connected = True

                elif isinstance(response, InferenceResponseToken):
                    print(f"   ✅ Token #{response.sequence}: {response.text}")
                    token_count += 1

                elif isinstance(response, InferenceResponseError):
                    print(f"   ❌ Error: {response.message}")
                    error_count += 1

                elif isinstance(response, InferenceResponseDone):
                    print(f"   ✔️  Done signal received")

                else:
                    print(f"   ℹ️  Unknown response type: {type(response)}")

                print()

        except Exception as e:
            print(f"❌ Streaming error: {e}")
            import traceback
            traceback.print_exc()
            return 1

        elapsed = time.time() - start_time

        # ===================================================================
        # Summary
        # ===================================================================
        print("┌─────────────────────────────────────────────────────────────────┐")
        print("│ Streaming Statistics                                           │")
        print("└─────────────────────────────────────────────────────────────────┘")
        print()
        print(f"  📊 Total responses received:  {response_count}")
        print(f"  📝 Token responses:           {token_count}")
        print(f"  ❌ Error responses:           {error_count}")
        print(f"  ⏱️  Total time:                {elapsed:.2f}s")
        print(f"  📈 Throughput:                {response_count/elapsed:.1f} responses/sec")
        print()

        print("=" * 70)
        print("✅ Bidirectional Streaming Demo Completed Successfully!")
        print("=" * 70)
        print()
        print("What was demonstrated:")
        print("  ✅ Client-side streaming: Sent multiple requests as AsyncIterable")
        print("  ✅ Server-side streaming: Received multiple responses as AsyncIterator")
        print("  ✅ Bidirectional: True concurrent streaming in both directions")
        print("  ✅ QUIC+TLS transport: Secure multiplexed streaming")
        print("  ✅ MessagePack serialization: Efficient binary protocol")
        print()
        print("Key differences from python_streaming_client.py:")
        print("  • That example: Multiple separate unary RPC calls")
        print("  • This example: Single bidirectional streaming RPC call")
        print("  • Benefits: Lower latency, less overhead, true streaming")
        print()
        print("Generated method used:")
        print("  • InferenceClient.generate(request_stream: AsyncIterable)")
        print("  • Returns: AsyncIterator[InferenceResponse]")
        print("=" * 70)

        return 0

    except ConnectionError as e:
        print()
        print(f"❌ Connection error: {e}")
        print()
        print("💡 Make sure the Rust cluster is running:")
        print()
        print("   # Terminal 1 - Director")
        print("   DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\\\")
        print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin director")
        print()
        print("   # Terminal 2 - Worker")
        print("   WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \\\\")
        print("     DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\\\")
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
