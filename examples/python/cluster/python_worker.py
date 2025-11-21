#!/usr/bin/env python3
"""
Python Worker for RpcNet Cluster

This demonstrates implementing an inference worker in Python using
the generated RPC bindings with multi-process support.
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__)))

import rpcnet
from inference.server import InferenceHandler, InferenceServer
from inference.types import (
    InferenceRequest,
    InferenceResponse,
    InferenceResponseConnected,
    InferenceResponseToken,
    InferenceResponseDone,
)


class PythonInferenceWorker(InferenceHandler):
    """Multi-process Python implementation of the Inference service
    
    This handler is serialized with cloudpickle and sent to worker processes,
    enabling true parallelism (no GIL contention).
    """

    def __init__(self, worker_label: str):
        self.worker_label = worker_label
        self.request_count = 0

    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        """Handle a single inference request
        
        This method runs in a worker process, providing true parallelism.
        Each worker process has its own Python interpreter and GIL.
        """
        import os
        self.request_count += 1
        
        # Show which process is handling the request
        response = InferenceResponseToken(
            text=f"Python worker '{self.worker_label}' (PID {os.getpid()}) processed: {request.prompt}",
            sequence=self.request_count
        )
        return response


async def main():
    """Start the multi-process Python worker with SWIM cluster"""
    # Configuration
    worker_label = os.getenv("WORKER_LABEL", "python-worker")
    worker_addr = os.getenv("WORKER_ADDR", "127.0.0.1:62002")
    director_addr = os.getenv("DIRECTOR_ADDR", "127.0.0.1:61000")
    cert_path = os.getenv("CERT_PATH", "certs/test_cert.pem")
    key_path = os.getenv("KEY_PATH", "certs/test_key.pem")
    num_processes = int(os.getenv("PROCESSES", os.cpu_count() or 1))

    print("=" * 70)
    print("🐍 Python Inference Worker with SWIM Cluster (Multi-Process)")
    print("=" * 70)
    print(f"Worker Label: {worker_label}")
    print(f"Worker Address: {worker_addr}")
    print(f"Director Address: {director_addr}")
    print(f"Certificate: {cert_path}")
    print(f"Key: {key_path}")
    print(f"Worker Processes: {num_processes}")
    print("=" * 70)
    print()

    # Create handler (will be serialized to workers with cloudpickle)
    handler = PythonInferenceWorker(worker_label)

    # Create config
    config = rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=worker_addr,
        server_name="localhost"
    )

    # Create server with multi-process support
    server = InferenceServer(handler, config)

    # Register handlers (will be serialized to workers)
    await server._register_handlers()

    # Bind the server (this sets socket_addr which is required for enable_cluster)
    print(f"🔌 Binding server to {worker_addr}...")
    await server.server.bind()
    print(f"✅ Server bound")

    # Enable cluster
    print(f"🌐 Creating QUIC client...")
    quic_client = await rpcnet.QuicClient.create(cert_path=cert_path)
    print(f"✅ QUIC client created")

    print(f"🔗 Enabling cluster, connecting to director at {director_addr}...")
    cluster_config = rpcnet.ClusterConfig()
    await server.server.enable_cluster(cluster_config, [director_addr], quic_client)
    print(f"✅ Cluster enabled")

    # Get cluster handle and update tags
    cluster = await server.server.cluster()
    if cluster:
        print(f"🏷️  Updating cluster tags...")
        await cluster.update_tags(
            [
                ("role", "worker"),
                ("label", worker_label),
                ("language", "python"),
            ]
        )
        print(f"✅ Tags updated")

    print(f"🚀 Python worker on {worker_addr} is now ready with {num_processes} processes!")
    print(f"✅ Starting to serve inference requests with true parallelism...")
    print(f"💡 Press Ctrl+C to stop")
    print()

    # Note: serve() will spawn worker processes, bind the server, and start serving.
    # Cluster functionality is enabled on the master process, workers handle requests.
    try:
        await server.serve()
    except (KeyboardInterrupt, asyncio.CancelledError):
        print("\n\n👋 Shutting down Python worker...")
        print("✅ Shutdown complete")


if __name__ == "__main__":
    asyncio.run(main())
