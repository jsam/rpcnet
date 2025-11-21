#!/usr/bin/env python3
"""
Client Streaming RPC Client - Stream of requests → Single response

The client sends a stream of messages, and the server responds with
a single aggregated result. Perfect for:
- File uploads (chunked transfer)
- Metrics/telemetry collection
- Batch data ingestion
- Log aggregation

Status: 🚧 Waiting for core library streaming support
"""

import asyncio
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))

from streamingservice.client import StreamingServiceClient
from streamingservice.types import ClientStreamRequest


async def main():
    server_addr = os.getenv("SERVER_ADDR", "127.0.0.1:50052")
    cert_path = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")
    
    print("=" * 70)
    print("📥 CLIENT STREAMING - Stream of Requests → Single Response")
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
        """Generator function that yields a stream of values"""
        values = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        
        print(f"📤 Sending stream of {len(values)} values...")
        for value in values:
            print(f"   → Sending: {value}")
            yield ClientStreamRequest(value=value)
            await asyncio.sleep(0.2)  # Simulate some delay
        print()
    
    try:
        # Send stream and get aggregated response
        response = await client.client_stream(send_stream())
        
        print(f"📥 Server response:")
        print(f"   Sum: {response.sum}")
        print(f"   Count: {response.count}")
        print(f"   Average: {response.sum / response.count:.2f}")
        print()
        print("=" * 70)
        print("✅ Client streaming demo completed!")
        print("=" * 70)
        
    except NotImplementedError as e:
        print(f"⚠️  Client streaming not yet implemented in core library")
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
