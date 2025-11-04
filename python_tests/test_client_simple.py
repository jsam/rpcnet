"""
Simple integration tests for RPC client functionality that actually work.
"""

import pytest
import _rpcnet


def test_serialization_roundtrip():
    """Test basic serialization roundtrip."""
    data = {"a": 10, "b": 20, "text": "hello"}
    serialized = _rpcnet.python_to_msgpack_py(data)
    deserialized = _rpcnet.msgpack_to_python_py(serialized)
    assert deserialized == data


def test_config_creation():
    """Test that we can create RPC config."""
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="127.0.0.1:8080",
        key_path="certs/test_key.pem",
    )
    assert config is not None


def test_server_creation(test_cert, test_key):
    """Test that we can create an RPC server."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)
    assert server is not None


@pytest.mark.asyncio
async def test_server_register(test_cert, test_key):
    """Test that we can register a handler."""
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    async def echo_handler(request_bytes: bytes) -> bytes:
        return request_bytes

    # Should not raise
    await server.register("echo", echo_handler)
