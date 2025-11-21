#!/usr/bin/env python3
"""
Bidirectional Streaming RPC Client - Stream ↔ Stream

Both client and server send streams of messages simultaneously.
The most flexible pattern. Perfect for:
- Chat applications
- Real-time collaboration
- Game state synchronization
- Live data transformation pipelines
- Interactive workflows

Status: 🚧 Waiting for core library streaming support
"""

import asyncio
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))

from streamingservice.client import StreamingServiceClient
from streamingservice.types import BidiStreamRequest


async def main():
    server_addr = os.getenv("SERVER_ADDR", "127.0.0.1:50052")
    cert_path = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")
    
    print("=" * 70)
    print("🔄 BIDIRECTIONAL STREAMING - Stream ↔ Stream")
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
    
    async def send_stream():
        """Generator that yields requests to send"""
        messages = [
            "Hello",
            "World",
            "Bidirectional",
            "Streaming",
            "Is",
            "Awesome",
            "RpcNet",
            "Python",
        ]
        
        print(f"📤 Starting to send {len(messages)} messages...\n")
        for msg in messages:
            print(f"   📤 Sending: {msg}")
            yield BidiStreamRequest(text=msg)
            await asyncio.sleep(0.3)  # Simulate some processing time
        print()
    
    try:
        # Start bidirectional streaming
        # Note: bidi_stream returns an async iterator, don't await it
        response_stream = client.bidi_stream(send_stream())
        
        print("📥 Receiving responses:\n")
        
        # Receive responses as they come in
        async for response in response_stream:
            print(f"   📥 Echo: '{response.echo}' → Reversed: '{response.reversed}'")
        
        print()
        print("=" * 70)
        print("✅ Bidirectional streaming demo completed!")
        print("=" * 70)
        
    except (NotImplementedError, TypeError, AttributeError) as e:
        print(f"⚠️  Bidirectional streaming not yet implemented in core library")
        print(f"   This client will work once streaming support is added\n")
        print("=" * 70)
        print("🚧 Waiting for streaming implementation")
        print("=" * 70)
    except Exception as e:
        error_msg = str(e).lower()
        if "timeout" in error_msg or "not found" in error_msg or "method" in error_msg:
            print(f"⚠️  Server doesn't support bidirectional streaming yet")
            print(f"   Error: {e}\n")
            print("=" * 70)
            print("🚧 Waiting for server-side streaming implementation")
            print("=" * 70)
        else:
            print(f"❌ Error: {e}")
            import traceback
            traceback.print_exc()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Interrupted by user")
