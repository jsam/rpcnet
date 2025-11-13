#!/usr/bin/env python3
"""
Client Streaming Example (N→1)

This example demonstrates a Python RPC server with a client streaming handler
that consumes multiple requests and returns a single response.

The handler aggregates incoming data, simulating scenarios like:
- File upload (receiving chunks)
- Data ingestion (batch processing)
- Aggregating metrics or statistics
"""

import asyncio
import sys
import _rpcnet


async def upload_file(request_stream):
    """
    Client streaming handler: consumes multiple requests, returns one response

    Receives chunks of data from the client and aggregates them
    """
    try:
        print("📥 Starting client stream: receiving chunks...")

        total_bytes = 0
        chunk_count = 0
        all_data = []

        # Consume all incoming requests
        async for chunk_bytes in request_stream:
            chunk = _rpcnet.msgpack_to_python_py(chunk_bytes)
            data = bytes(chunk["data"])  # Convert list back to bytes

            all_data.append(data)
            total_bytes += len(data)
            chunk_count += 1

            print(f"  ⬅️  Received chunk {chunk_count}: {len(data)} bytes")

        # Combine all chunks
        combined_data = b"".join(all_data)

        print(f"✅ Client stream complete: received {chunk_count} chunks, {total_bytes} bytes total")

        # Return single response with statistics
        response = {
            "status": "success",
            "chunks_received": chunk_count,
            "total_bytes": total_bytes,
            "data_hash": hash(combined_data) & 0xFFFFFFFF  # Simple hash for verification
        }

        return _rpcnet.python_to_msgpack_py(response)

    except Exception as e:
        print(f"❌ Error in upload_file: {e}")
        import traceback
        traceback.print_exc()

        error_response = {
            "status": "error",
            "message": str(e)
        }
        return _rpcnet.python_to_msgpack_py(error_response)


async def main():
    print("=" * 60)
    print("Client Streaming Example (N→1)")
    print("=" * 60)
    print()

    # Create server configuration
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        key_path="certs/test_key.pem",
        bind_addr="127.0.0.1:9002",
        server_name="localhost",
        timeout_secs=30
    )

    print("🔧 Creating RPC server...")
    server = _rpcnet.RpcServer(config)

    print("📝 Registering client streaming handler 'upload_file'...")
    await server.register_client_streaming("upload_file", upload_file)

    print(f"🚀 Server listening on 127.0.0.1:9002")
    print()
    print("To test this server, run a client that sends multiple chunks to 'upload_file':")
    print('  For each chunk: {"data": [byte array]}')
    print()
    print("The server will aggregate all chunks and return statistics.")
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
