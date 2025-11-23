#!/usr/bin/env python3
import asyncio
import sys
import os
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "generated"))

import rpcnet
from benchmarkservice import BenchmarkServiceHandler, BenchmarkServiceServer
from benchmarkservice.types import *

class MyBenchmarkHandler(BenchmarkServiceHandler):
    async def ping(self, request: PingRequest) -> PingResponse:
        return PingResponse(
            payload=request.payload,
            server_time_ns=time.time_ns()
        )
    
    async def process(self, request: BenchmarkRequest) -> BenchmarkResponse:
        return BenchmarkResponse(
            echo=request.message,
            doubled=request.value * 2,
            server_time_ns=time.time_ns()
        )
    
    async def noop(self, request: NoopRequest) -> NoopResponse:
        return NoopResponse(success=True)

async def main():
    bind_addr = os.getenv("BIND_ADDR", "127.0.0.1:50051")
    cert_path = "../../../certs/test_cert.pem"
    key_path = "../../../certs/test_key.pem"

    print("=" * 70)
    print("🚀 RpcNet Server")
    print("=" * 70)
    print(f"Address: {bind_addr}")
    print("=" * 70)

    config = rpcnet.RpcConfig(
        cert_path=cert_path,
        bind_addr=bind_addr,
        key_path=key_path,
    )

    handler = MyBenchmarkHandler()
    server = BenchmarkServiceServer(handler, config)
    print("✅ Server ready")
    await server.serve()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Stopped")
