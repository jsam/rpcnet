#!/usr/bin/env python3
"""
Test client for Python Worker

This demonstrates calling the Python inference worker directly.
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

import _rpcnet
from inference.client import InferenceClient
from inference.types import InferenceRequest


async def main():
    """Test the Python worker"""
    # Configuration
    worker_addr = os.getenv("WORKER_ADDR", "127.0.0.1:62002")
    cert_path = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")

    print("=" * 70)
    print("🧪 Python Inference Worker Test Client")
    print("=" * 70)
    print(f"Worker Address: {worker_addr}")
    print(f"Certificate: {cert_path}")
    print("=" * 70)
    print()

    print(f"🔌 Connecting to Python worker at {worker_addr}...")
    client = await InferenceClient.connect(
        addr=worker_addr,
        cert_path=cert_path,
        server_name="localhost",
        timeout_secs=10
    )
    print(f"✅ Connected!")
    print()

    # Test a few requests
    test_prompts = [
        "Hello, Python worker!",
        "What is 2+2?",
        "Tell me a joke",
    ]

    for i, prompt in enumerate(test_prompts, 1):
        print(f"📤 Request #{i}: {prompt}")

        request = InferenceRequest(
            connection_id=f"test-{i}",
            prompt=prompt
        )

        try:
            response = await client.infer(request)
            print(f"📥 Response: {response}")
            print(f"   Type: {type(response).__name__}")
            if hasattr(response, 'text'):
                print(f"   Text: {response.text}")
            if hasattr(response, 'sequence'):
                print(f"   Sequence: {response.sequence}")
            print()
        except Exception as e:
            print(f"❌ Error: {e}")
            import traceback
            traceback.print_exc()
            print()

    print("=" * 70)
    print("✅ Test completed!")
    print("=" * 70)


if __name__ == "__main__":
    asyncio.run(main())
