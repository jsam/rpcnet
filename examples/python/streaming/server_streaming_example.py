#!/usr/bin/env python3
"""
Server Streaming Example (1→N)

This example demonstrates a Python RPC server with a server streaming handler
that yields multiple responses for a single request.

The handler generates a stream of numbers, simulating a scenario like:
- Streaming log entries
- Sending multiple search results
- Real-time data feed
"""

import asyncio
import sys
import _rpcnet


async def stream_numbers(request_bytes: bytes):
    """
    Server streaming handler: yields multiple responses for one request

    Takes a request with a count, yields that many numbers with delays
    """
    try:
        # Deserialize request to get the count
        request = _rpcnet.msgpack_to_python_py(request_bytes)
        count = request.get("count", 10)

        print(f"📤 Starting server stream: will yield {count} numbers")

        # Yield multiple responses
        for i in range(count):
            await asyncio.sleep(0.1)  # Simulate processing delay

            response = {
                "index": i,
                "value": i * i,  # Square of the index
                "timestamp": asyncio.get_event_loop().time()
            }

            # Serialize and yield
            response_bytes = _rpcnet.python_to_msgpack_py(response)
            print(f"  ➡️  Yielding item {i}: {response}")
            yield response_bytes

        print(f"✅ Server stream complete: yielded {count} items")

    except Exception as e:
        print(f"❌ Error in stream_numbers: {e}")
        import traceback
        traceback.print_exc()


async def main():
    print("=" * 60)
    print("Server Streaming Example (1→N)")
    print("=" * 60)
    print()

    # Create server configuration
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        key_path="certs/test_key.pem",
        bind_addr="127.0.0.1:9001",
        server_name="localhost",
        timeout_secs=30
    )

    print("🔧 Creating RPC server...")
    server = _rpcnet.RpcServer(config)

    print("📝 Registering server streaming handler 'stream_numbers'...")
    await server.register_server_streaming("stream_numbers", stream_numbers)

    print(f"🚀 Server listening on 127.0.0.1:9001")
    print()
    print("To test this server, run a client that calls 'stream_numbers' with:")
    print('  {"count": 5}')
    print()
    print("The server will yield 5 responses, one for each number.")
    print()
    print("Press Ctrl+C to stop the server")
    print("-" * 60)
    print()

    try:
        # Start serving (blocks until shutdown)
        await server.serve()
    except KeyboardInterrupt:
        print("\n⏹️  Server stopped")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n⏹️  Exiting")
        sys.exit(0)
