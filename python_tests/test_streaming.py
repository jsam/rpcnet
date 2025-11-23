"""
Tests for streaming RPC functionality.
"""

import pytest
import asyncio
import _rpcnet


@pytest.mark.asyncio
async def test_server_streaming(test_cert, test_key):
    """Test server streaming (one request, multiple responses)."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that sends 5 responses
    async def count_handler(request_bytes: bytes) -> bytes:
        # For server streaming, handler should return an async generator
        # or we need to modify the test to use the actual streaming API
        # For now, this is a placeholder showing the test structure
        pass

    await server.register("count", count_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Call server streaming method
        request = _rpcnet.python_to_msgpack_py({"count": 5})
        stream = await client.call_server_streaming("count", request)

        # Collect all responses
        responses = []
        async for response_bytes in stream:
            response = _rpcnet.msgpack_to_python_py(response_bytes)
            responses.append(response)

        # Verify we got 5 responses
        assert len(responses) == 5
        for i, response in enumerate(responses):
            assert response == {"value": i}
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_server_streaming_collect(test_cert, test_key):
    """Test server streaming with collect() method."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler for streaming
    async def range_handler(request_bytes: bytes) -> bytes:
        pass  # Placeholder

    await server.register("range", range_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Call streaming method and collect all at once
        request = _rpcnet.python_to_msgpack_py({"n": 10})
        stream = await client.call_server_streaming("range", request)
        all_responses = await stream.collect()

        # Verify we got all 10 responses
        assert len(all_responses) == 10
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_client_streaming(test_cert, test_key):
    """Test client streaming (multiple requests, one response)."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that sums all incoming values
    async def sum_handler(request_stream):
        # Placeholder for client streaming handler
        pass

    await server.register("sum", sum_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Send multiple requests
        requests = []
        for i in range(1, 6):  # 1, 2, 3, 4, 5
            request = _rpcnet.python_to_msgpack_py({"value": i})
            requests.append(request)

        # Get single response
        response_bytes = await client.call_client_streaming("sum", requests)
        response = _rpcnet.msgpack_to_python_py(response_bytes)

        # Verify sum is correct (1+2+3+4+5 = 15)
        assert response == {"sum": 15}
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_bidirectional_streaming(test_cert, test_key):
    """Test bidirectional streaming (multiple requests, multiple responses)."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that echoes each request with modification
    async def echo_transform_handler(request_stream):
        # Placeholder for bidirectional streaming handler
        pass

    await server.register("echo_transform", echo_transform_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Send multiple requests
        requests = []
        for i in range(5):
            request = _rpcnet.python_to_msgpack_py({"id": i, "value": i * 2})
            requests.append(request)

        # Get response stream
        stream = await client.call_streaming("echo_transform", requests)

        # Collect responses
        responses = []
        async for response_bytes in stream:
            response = _rpcnet.msgpack_to_python_py(response_bytes)
            responses.append(response)

        # Verify we got all responses
        assert len(responses) == 5
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_streaming_large_data(test_cert, test_key):
    """Test streaming with large payloads."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that streams large chunks
    async def large_chunks_handler(request_bytes: bytes):
        pass  # Placeholder

    await server.register("large_chunks", large_chunks_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Request stream of large chunks
        request = _rpcnet.python_to_msgpack_py({"chunk_size": 1024 * 1024, "count": 5})
        stream = await client.call_server_streaming("large_chunks", request)

        # Verify we can handle large streaming data
        total_bytes = 0
        async for response_bytes in stream:
            total_bytes += len(response_bytes)

        # Should have received ~5MB
        assert total_bytes >= 5 * 1024 * 1024
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_streaming_early_termination(test_cert, test_key):
    """Test that we can stop iterating over a stream early."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that streams many items
    async def infinite_handler(request_bytes: bytes):
        pass  # Placeholder

    await server.register("infinite", infinite_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Start streaming
        request = _rpcnet.python_to_msgpack_py({"limit": 1000})
        stream = await client.call_server_streaming("infinite", request)

        # Only take first 5 items
        count = 0
        async for response_bytes in stream:
            count += 1
            if count >= 5:
                break

        # Verify we stopped early
        assert count == 5
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_empty_client_stream(test_cert, test_key):
    """Test client streaming with empty request list."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that handles empty stream
    async def count_handler(request_stream):
        pass  # Placeholder

    await server.register("count_requests", count_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Send empty stream
        response_bytes = await client.call_client_streaming("count_requests", [])
        response = _rpcnet.msgpack_to_python_py(response_bytes)

        # Should get count of 0
        assert response == {"count": 0}
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_streaming_error_handling(test_cert, test_key):
    """Test error handling in streaming."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that errors after a few items
    async def failing_handler(request_bytes: bytes):
        pass  # Placeholder

    await server.register("failing_stream", failing_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Start streaming that will error
        request = _rpcnet.python_to_msgpack_py({"error_after": 3})
        stream = await client.call_server_streaming("failing_stream", request)

        # Should get error while iterating
        with pytest.raises(_rpcnet.RpcError):
            async for response_bytes in stream:
                pass  # Should error before completing
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass
