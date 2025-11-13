#!/usr/bin/env python3
"""
Python Worker for RpcNet Cluster

This demonstrates implementing an inference worker in Python using
the generated RPC bindings.
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

import _rpcnet
from inference.server import InferenceHandler, InferenceServer
from inference.types import (
    InferenceRequest,
    InferenceResponse,
    InferenceResponseConnected,
    InferenceResponseToken,
    InferenceResponseDone,
)


class PythonInferenceWorker(InferenceHandler):
    """Simple Python implementation of the Inference service"""

    def __init__(self, worker_label: str):
        self.worker_label = worker_label
        self.request_count = 0

    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        """Handle a single inference request"""
        self.request_count += 1

        print(f"📥 Received inference request #{self.request_count}")
        print(f"   Connection ID: {request.connection_id}")
        print(f"   Prompt: {request.prompt}")

        # Simulate processing
        await asyncio.sleep(0.1)

        # Return a simple token response
        response = InferenceResponseToken(
            text=f"Python worker '{self.worker_label}' processed: {request.prompt}",
            sequence=self.request_count
        )

        print(f"📤 Sending response: {response.text[:50]}...")
        return response


async def main():
    """Start the Python worker"""
    # Configuration
    worker_label = os.getenv("WORKER_LABEL", "python-worker")
    worker_addr = os.getenv("WORKER_ADDR", "127.0.0.1:62002")
    cert_path = os.getenv("CERT_PATH", "certs/test_cert.pem")
    key_path = os.getenv("KEY_PATH", "certs/test_key.pem")

    print("=" * 70)
    print("🐍 Python Inference Worker")
    print("=" * 70)
    print(f"Worker Label: {worker_label}")
    print(f"Worker Address: {worker_addr}")
    print(f"Certificate: {cert_path}")
    print(f"Key: {key_path}")
    print("=" * 70)
    print()

    # Create handler
    handler = PythonInferenceWorker(worker_label)

    # Create config
    config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=worker_addr,
        server_name="localhost"
    )

    # Create and start server
    server = InferenceServer(handler, config)

    print(f"🚀 Starting Python worker on {worker_addr}...")
    print(f"✅ Ready to serve inference requests!")
    print(f"💡 Press Ctrl+C to stop")
    print()

    try:
        await server.serve()
    except (KeyboardInterrupt, asyncio.CancelledError):
        print("\n\n👋 Shutting down Python worker...")
        print(f"📊 Total requests served: {handler.request_count}")
        print("✅ Shutdown complete")


if __name__ == "__main__":
    asyncio.run(main())
