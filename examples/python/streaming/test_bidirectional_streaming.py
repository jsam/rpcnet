#!/usr/bin/env python3
"""
Client to test the Bidirectional Streaming example

This connects to the bidirectional streaming server and sends/receives messages concurrently.
"""

import asyncio
import _rpcnet


async def main():
    print("=" * 60)
    print("Testing Bidirectional Streaming (N→M)")
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

    print("🔌 Connecting to server at 127.0.0.1:9003...")
    client = await _rpcnet.RpcClient.connect("127.0.0.1:9003", config)
    print("✅ Connected!")
    print()

    # Prepare list of requests
    messages = [
        "Hello",
        "How are you?",
        "This is bidirectional",
        "streaming in action!",
        "Goodbye",
    ]

    request_list = []
    for i, message in enumerate(messages, 1):
        print(f"  📤 Preparing: {message}")
        request = {"message": message}
        request_list.append(_rpcnet.python_to_msgpack_py(request))

    print()
    print("🔄 Starting bidirectional stream...")
    print()

    # Call bidirectional streaming method
    response_stream = await client.call_streaming("echo_transform", request_list)

    # Consume responses as they arrive
    count = 0
    async for response_bytes in response_stream:
        response = _rpcnet.msgpack_to_python_py(response_bytes)
        count += 1
        print(f"  📥 [{count}] Received: {response['transformed']}")

    print()
    print(f"✅ Bidirectional streaming complete! Exchanged {count} messages")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n⏹️  Interrupted")
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
