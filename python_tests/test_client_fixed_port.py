"""
Integration tests for RPC client with fixed ports (working version).
"""

import pytest
import asyncio
import _rpcnet

# Use fixed ports starting from 19000 to avoid conflicts
BASE_PORT = 19000


def get_next_port():
    """Get next available test port."""
    global BASE_PORT
    port = BASE_PORT
    BASE_PORT += 1
    return port


@pytest.mark.asyncio
async def test_basic_echo(test_cert, test_key):
    """Test basic echo RPC call."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("echo", echo_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)  # Give server time to start

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

        # Make call
        request = b"Hello, RpcNet!"
        response = await client.call("echo", request)
        assert response == request

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_structured_data(test_cert, test_key):
    """Test RPC call with structured data."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def double_handler(request_bytes: bytes) -> bytes:
        data = _rpcnet.msgpack_to_python_py(request_bytes)
        result = {"result": data["value"] * 2}
        return _rpcnet.python_to_msgpack_py(result)

    await server.register("double", double_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

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
async def test_multiple_calls(test_cert, test_key):
    """Test multiple sequential RPC calls."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("echo", echo_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

        # Make multiple calls
        for i in range(5):
            request = f"Message {i}".encode()
            response = await client.call("echo", request)
            assert response == request

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_timeout(test_cert, test_key):
    """Test RPC call with timeout."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def quick_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("quick", quick_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

        # Call with timeout
        request = b"Quick test"
        response = await client.call_with_timeout("quick", request, 5.0)
        assert response == request

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_large_payload(test_cert, test_key):
    """Test RPC call with large payload."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("echo", echo_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

        # Test with 100KB payload
        large_data = b"X" * (100 * 1024)
        response = await client.call("echo", large_data)
        assert response == large_data
        assert len(response) == 100 * 1024

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass


@pytest.mark.asyncio
async def test_binary_data(test_cert, test_key):
    """Test RPC call with binary data."""
    port = get_next_port()

    # Setup server
    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{port}",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(server_config)

    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("echo", echo_handler)

    # Start server
    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.2)

    try:
        # Connect client
        client_config = _rpcnet.RpcConfig(
            cert_path=test_cert,
            bind_addr="0.0.0.0:0",
            server_name="localhost",
        )
        client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{port}", client_config)

        # Test with all byte values
        binary_data = bytes(range(256))
        response = await client.call("echo", binary_data)
        assert response == binary_data

    finally:
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass
