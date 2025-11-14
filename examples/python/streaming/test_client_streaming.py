#!/usr/bin/env python3
"""
Client to test the Client Streaming example

This connects to the client streaming server and sends multiple chunks.
"""

import asyncio
import _rpcnet


async def main():
    print("=" * 60)
    print("Testing Client Streaming (N→1)")
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

    print("🔌 Connecting to server at 127.0.0.1:9002...")
    client = await _rpcnet.RpcClient.connect("127.0.0.1:9002", config)
    print("✅ Connected!")
    print()

    # Prepare the list of requests
    chunks = [
        b"Hello, ",
        b"this is ",
        b"a test ",
        b"of client ",
        b"streaming!",
    ]

    print("📤 Sending stream of chunks...")
    print()

    # Create list of serialized requests
    request_list = []
    for i, chunk in enumerate(chunks, 1):
        print(f"  📤 Preparing chunk {i}: {chunk.decode()}")
        # Wrap in dict with 'data' key as list of bytes
        request = {"data": list(chunk)}
        request_list.append(_rpcnet.python_to_msgpack_py(request))

    # Call client streaming method with list of requests
    response_bytes = await client.call_client_streaming("upload_file", request_list)

    response = _rpcnet.msgpack_to_python_py(response_bytes)
    print()
    print(f"📥 Received final response: {response}")
    print()
    print("✅ Client streaming complete!")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n⏹️  Interrupted")
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
