"""
Pytest configuration and fixtures for Python bindings tests.
"""

import pytest
import pytest_asyncio
import asyncio
import os
import sys
from pathlib import Path

# Add the built module to path
# This assumes maturin develop or wheel installation
try:
    import _rpcnet
except ImportError:
    pytest.skip("_rpcnet module not installed. Run 'maturin develop' first.", allow_module_level=True)


@pytest.fixture(scope="session")
def event_loop_policy():
    """Set event loop policy for async tests."""
    return asyncio.DefaultEventLoopPolicy()


@pytest.fixture
def certs_dir():
    """Path to test certificates directory."""
    return Path(__file__).parent.parent / "certs"


@pytest.fixture
def test_cert(certs_dir):
    """Path to test certificate."""
    cert_path = certs_dir / "test_cert.pem"
    if not cert_path.exists():
        pytest.skip(f"Test certificate not found at {cert_path}")
    return str(cert_path)


@pytest.fixture
def test_key(certs_dir):
    """Path to test private key."""
    key_path = certs_dir / "test_key.pem"
    if not key_path.exists():
        pytest.skip(f"Test key not found at {key_path}")
    return str(key_path)


@pytest_asyncio.fixture
async def rpc_server_and_client(test_cert, test_key):
    """Create and start a test RPC server with a connected client."""
    import _rpcnet

    # Use fixed port for testing to avoid port discovery issues
    server_port = 18080

    server_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr=f"127.0.0.1:{server_port}",
        key_path=test_key,
    )

    server = _rpcnet.RpcServer(server_config)

    # Register a simple echo handler
    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    await server.register("echo", echo_handler)

    # Start server in background as a non-awaited task
    import asyncio
    server_future = server.serve()
    server_task = asyncio.ensure_future(server_future)

    # Give server a moment to start
    await asyncio.sleep(0.3)

    # Create client
    client_config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="0.0.0.0:0",
        server_name="localhost",
    )
    client = await _rpcnet.RpcClient.connect(f"127.0.0.1:{server_port}", client_config)

    yield (server, client)

    # Cleanup
    server_task.cancel()
    try:
        await server_task
    except (asyncio.CancelledError, Exception):
        pass


# Convenience fixtures for backward compatibility
@pytest_asyncio.fixture
async def rpc_server(rpc_server_and_client):
    """Get just the server from the server_and_client fixture."""
    server, _ = rpc_server_and_client
    return server


@pytest_asyncio.fixture
async def rpc_client(rpc_server_and_client):
    """Get just the client from the server_and_client fixture."""
    _, client = rpc_server_and_client
    return client


# Pytest async support
pytest_plugins = ('pytest_asyncio',)
