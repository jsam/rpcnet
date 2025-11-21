#!/usr/bin/env python3
"""
Simple RPC client demonstrating basic unary pattern.

Note: Full streaming support (server_stream, client_stream, bidi_stream) 
will be demonstrated when the core library adds streaming methods.
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
    
    print("╔" + "=" * 68 + "╗")
    print("║" + " " * 20 + "RpcNet Python Client Demo" + " " * 23 + "║")
    print("╚" + "=" * 68 + "╝")
    print(f"\nServer: {server_addr}")
    print(f"Certificate: {cert_path}\n")
    
    # Connect to server
    print("🔌 Connecting to server...")
    client = await StreamingServiceClient.connect(
        addr=server_addr,
        cert_path=cert_path,
        server_name="localhost",
        timeout_secs=5
    )
    print("✅ Connected!\n")
    
    try:
        # Test unary RPC
        print("=" * 70)
        print("📨 Testing Unary RPC (single request/response)")
        print("=" * 70)
        
        for i in range(3):
            request = UnaryRequest(message=f"Hello from Python client #{i+1}!")
            print(f"\n📤 Sending: {request.message}")
            
            response = await client.unary(request)
            
            print(f"📥 Response: {response.reply}")
            print(f"   Timestamp: {response.timestamp}")
            
            await asyncio.sleep(0.5)
        
        print("\n" + "=" * 70)
        print("🎉 Demo completed successfully!")
        print("=" * 70)
        
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    asyncio.run(main())
