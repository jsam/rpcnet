#!/usr/bin/env python3
"""
Bidirectional Streaming Example (N→M)

This example demonstrates a Python RPC server with a bidirectional streaming handler
that consumes multiple requests and yields multiple responses.

The handler processes incoming data and yields transformed results, simulating scenarios like:
- Real-time chat
- Live data transformation/filtering
- Interactive processing pipeline
"""

import asyncio
import sys
import _rpcnet


async def echo_transform(request_stream):
    """
    Bidirectional streaming handler: consumes multiple requests, yields multiple responses

    Takes incoming messages, transforms them, and yields results in real-time
    """
    try:
        print("🔄 Starting bidirectional stream...")

        message_count = 0

        # Process incoming requests and yield responses
        async for request_bytes in request_stream:
            message_count += 1

            # Deserialize incoming message
            request = _rpcnet.msgpack_to_python_py(request_bytes)
            message = request.get("message", "")

            print(f"  ⬅️  Received message {message_count}: {message}")

            # Transform: uppercase, add prefix, and echo back
            transformed = f"ECHO [{message_count}]: {message.upper()}"

            # Simulate processing delay
            await asyncio.sleep(0.05)

            # Yield transformed response
            response = {
                "index": message_count,
                "original": message,
                "transformed": transformed,
                "timestamp": asyncio.get_event_loop().time()
            }

            response_bytes = _rpcnet.python_to_msgpack_py(response)
            print(f"  ➡️  Yielding response {message_count}: {transformed}")

            yield response_bytes

        print(f"✅ Bidirectional stream complete: processed {message_count} messages")

    except Exception as e:
        print(f"❌ Error in echo_transform: {e}")
        import traceback
        traceback.print_exc()


async def main():
    print("=" * 60)
    print("Bidirectional Streaming Example (N→M)")
    print("=" * 60)
    print()

    # Create server configuration
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        key_path="certs/test_key.pem",
        bind_addr="127.0.0.1:9003",
        server_name="localhost",
        timeout_secs=30
    )

    print("🔧 Creating RPC server...")
    server = _rpcnet.RpcServer(config)

    print("📝 Registering bidirectional streaming handler 'echo_transform'...")
    await server.register_bidirectional("echo_transform", echo_transform)

    print(f"🚀 Server listening on 127.0.0.1:9003")
    print()
    print("To test this server, run a client that sends messages to 'echo_transform':")
    print('  For each message: {"message": "your text here"}')
    print()
    print("The server will transform each message and yield it back immediately.")
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
