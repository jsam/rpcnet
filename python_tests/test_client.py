"""
Integration tests for RPC client functionality.
"""

import pytest
import asyncio
import _rpcnet


@pytest.mark.asyncio
async def test_basic_rpc_call(rpc_server, rpc_client):
    """Test a basic RPC call with echo handler."""
    request = b"Hello, RpcNet!"
    response = await rpc_client.call("echo", request)
    assert response == request


@pytest.mark.asyncio
async def test_rpc_call_with_json_like_data(rpc_server, test_cert, test_key):
    """Test RPC call with structured data (simulating JSON)."""
    # Create server with custom handler
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that doubles a number
    async def double_handler(request_bytes: bytes) -> bytes:
        data = _rpcnet.msgpack_to_python_py(request_bytes)
        result = {"result": data["value"] * 2}
        return _rpcnet.python_to_msgpack_py(result)

    await server.register("double", double_handler)

    # Start server in background
    server_task = asyncio.create_task(server.serve())

    # Give server time to start
    await asyncio.sleep(0.1)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect("127.0.0.1:0", client_config)

        # Make call
        request = _rpcnet.python_to_msgpack_py({"value": 21})
        response_bytes = await client.call("double", request)
        response = _rpcnet.msgpack_to_python_py(response_bytes)

        assert response == {"result": 42}
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_rpc_call_with_timeout(rpc_server, rpc_client):
    """Test RPC call with custom timeout."""
    request = b"Quick test"
    response = await rpc_client.call_with_timeout("echo", request, 5.0)
    assert response == request


@pytest.mark.asyncio
async def test_rpc_timeout_error(test_cert, test_key):
    """Test that slow handlers cause timeout errors."""
    # Create server with slow handler
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Handler that takes 2 seconds
    async def slow_handler(request_bytes: bytes) -> bytes:
        await asyncio.sleep(2)
        return request_bytes

    await server.register("slow", slow_handler)

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

        # Make call with 0.5 second timeout (should fail)
        with pytest.raises(_rpcnet.TimeoutError):
            await client.call_with_timeout("slow", b"test", 0.5)
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_multiple_concurrent_calls(rpc_server, rpc_client):
    """Test multiple concurrent RPC calls."""
    # Make 10 concurrent calls
    tasks = []
    for i in range(10):
        request = f"Request {i}".encode()
        task = rpc_client.call("echo", request)
        tasks.append((task, request))

    # Wait for all to complete
    results = await asyncio.gather(*[t[0] for t in tasks])

    # Verify all responses match requests
    for result, (_, expected) in zip(results, tasks):
        assert result == expected


@pytest.mark.asyncio
async def test_large_payload(rpc_server, rpc_client):
    """Test RPC call with large payload (1MB)."""
    # Create 1MB payload
    large_data = b"X" * (1024 * 1024)
    response = await rpc_client.call("echo", large_data)
    assert response == large_data
    assert len(response) == 1024 * 1024


@pytest.mark.asyncio
async def test_empty_payload(rpc_server, rpc_client):
    """Test RPC call with empty payload."""
    response = await rpc_client.call("echo", b"")
    assert response == b""


@pytest.mark.asyncio
async def test_binary_data(rpc_server, rpc_client):
    """Test RPC call with binary data (not UTF-8)."""
    binary_data = bytes(range(256))
    response = await rpc_client.call("echo", binary_data)
    assert response == binary_data


@pytest.mark.asyncio
async def test_multiple_method_handlers(test_cert, test_key):
    """Test server with multiple registered handlers."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    # Register multiple handlers
    async def add_handler(request_bytes: bytes) -> bytes:
        data = _rpcnet.msgpack_to_python_py(request_bytes)
        result = {"sum": data["a"] + data["b"]}
        return _rpcnet.python_to_msgpack_py(result)

    async def multiply_handler(request_bytes: bytes) -> bytes:
        data = _rpcnet.msgpack_to_python_py(request_bytes)
        result = {"product": data["a"] * data["b"]}
        return _rpcnet.python_to_msgpack_py(result)

    await server.register("add", add_handler)
    await server.register("multiply", multiply_handler)

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

        # Test add
        request = _rpcnet.python_to_msgpack_py({"a": 10, "b": 20})
        response_bytes = await client.call("add", request)
        response = _rpcnet.msgpack_to_python_py(response_bytes)
        assert response == {"sum": 30}

        # Test multiply
        request = _rpcnet.python_to_msgpack_py({"a": 5, "b": 7})
        response_bytes = await client.call("multiply", request)
        response = _rpcnet.msgpack_to_python_py(response_bytes)
        assert response == {"product": 35}
    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_connection_error_invalid_address():
    """Test that connecting to invalid address raises ConnectionError."""
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="0.0.0.0:0",
        server_name="localhost",
    )

    # Try to connect to address where no server is running
    with pytest.raises(_rpcnet.ConnectionError):
        await asyncio.wait_for(
            _rpcnet.RpcClient.connect("127.0.0.1:9999", config),
            timeout=2.0
        )
