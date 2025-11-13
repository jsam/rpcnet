#!/usr/bin/env python3
"""
Client to test the Server Streaming example

This connects to the server streaming server and requests a stream of numbers.
"""

import asyncio
import _rpcnet


async def main():
    print("=" * 60)
    print("Testing Server Streaming (1→N)")
    print("=" * 60)
    print()

    # Create client configuration
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        key_path="certs/test_key.pem",
        bind_addr="0.0.0.0:0",
        server_name="localhost",
        timeout_secs=30
    )

    print("🔌 Connecting to server at 127.0.0.1:9001...")
    client = await _rpcnet.RpcClient.connect("127.0.0.1:9001", config)
    print("✅ Connected!")
    print()

    # Prepare request
    request = {"count": 5}
    request_bytes = _rpcnet.python_to_msgpack_py(request)

    print(f"📤 Sending request: {request}")
    print("📥 Receiving stream...")
    print()

    # Call server streaming method
    response_stream = await client.call_server_streaming("stream_numbers", request_bytes)

    # Consume the stream
    count = 0
    async for response_bytes in response_stream:
        response = _rpcnet.msgpack_to_python_py(response_bytes)
        count += 1
        print(f"  [{count}] Received: {response}")

    print()
    print(f"✅ Stream complete! Received {count} responses")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n⏹️  Interrupted")
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
