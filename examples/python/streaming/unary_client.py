#!/usr/bin/env python3
"""
Unary RPC Client - Single request → Single response

This is the most basic RPC pattern where the client sends one request
and receives one response. Perfect for simple queries and commands.

Status: ✅ Working
"""

import asyncio
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))

from streamingservice.client import StreamingServiceClient
from streamingservice.types import UnaryRequest


async def main():
    server_addr = os.getenv("SERVER_ADDR", "127.0.0.1:50052")
    cert_path = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")
    
    print("=" * 70)
    print("📨 UNARY RPC - Single Request → Single Response")
    print("=" * 70)
    print(f"Server: {server_addr}\n")
    
    # Connect to server
    client = await StreamingServiceClient.connect(
        addr=server_addr,
        cert_path=cert_path,
        server_name="localhost",
        timeout_secs=5
    )
    print("✅ Connected to server\n")
    
    # Send multiple unary requests
    for i in range(5):
        request = UnaryRequest(message=f"Message #{i+1}: Hello from unary client!")
        
        print(f"📤 Sending: {request.message}")
        response = await client.unary(request)
        
        print(f"📥 Response: {response.reply}")
        print(f"   Timestamp: {response.timestamp}")
        print()
        
        await asyncio.sleep(0.3)
    
    print("=" * 70)
    print("✅ Unary RPC demo completed!")
    print("=" * 70)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
