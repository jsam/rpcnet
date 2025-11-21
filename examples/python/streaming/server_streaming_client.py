#!/usr/bin/env python3
"""
Server Streaming RPC Client - Single request → Stream of responses

The client sends one request, and the server responds with a stream
of messages. Perfect for:
- Progress updates for long operations
- Real-time data feeds (stock prices, sensor data)
- Paginated results
- Event notifications

Status: 🚧 Waiting for core library streaming support
"""

import asyncio
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))

from streamingservice.client import StreamingServiceClient
from streamingservice.types import ServerStreamRequest


async def main():
    server_addr = os.getenv("SERVER_ADDR", "127.0.0.1:50052")
    cert_path = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")
    
    print("=" * 70)
    print("📤 SERVER STREAMING - Single Request → Stream of Responses")
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
    
    # Request a stream of 10 items
    request = ServerStreamRequest(count=10, prefix="Item")
    print(f"📨 Requesting {request.count} items with prefix '{request.prefix}'...")
    print()
    
    try:
        # Receive stream of responses
        # Note: server_stream returns an async iterator, don't await it
        response_stream = client.server_stream(request)
        
        async for response in response_stream:
            print(f"📥 Received: {response.item} (index: {response.index})")
        
        print()
        print("=" * 70)
        print("✅ Server streaming demo completed!")
        print("=" * 70)
        
    except (NotImplementedError, TypeError) as e:
        print(f"⚠️  Server streaming not yet implemented in core library")
        print(f"   This client will work once streaming support is added\n")
        print("=" * 70)
        print("🚧 Waiting for streaming implementation")
        print("=" * 70)
    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Interrupted by user")
