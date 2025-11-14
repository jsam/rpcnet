"""
Type stubs for _rpcnet module.

This file provides type hints for the compiled Rust extension module.
"""

from typing import Any, AsyncIterator, Awaitable, Optional, List

# Configuration
class RpcConfig:
    """Configuration for RPC client/server with TLS settings."""

    def __init__(
        self,
        cert_path: str,
        bind_addr: str,
        key_path: Optional[str] = None,
        server_name: Optional[str] = None,
        timeout_secs: Optional[int] = None,
    ) -> None:
        """
        Create RPC configuration.

        Args:
            cert_path: Path to TLS certificate file
            bind_addr: Address to bind to (e.g., "127.0.0.1:8080")
            key_path: Optional path to private key file
            server_name: Optional server name for TLS verification
            timeout_secs: Optional default timeout in seconds
        """
        ...

# Client
class RpcClient:
    """Async RPC client for making requests over QUIC+TLS."""

    @staticmethod
    async def connect(addr: str, config: RpcConfig) -> "RpcClient":
        """
        Connect to an RPC server.

        Args:
            addr: Server address (e.g., "127.0.0.1:8080")
            config: RPC configuration with TLS settings

        Returns:
            Connected RPC client

        Raises:
            ConnectionError: If connection fails
            TlsError: If TLS setup fails
        """
        ...

    async def call(self, method: str, params: bytes) -> bytes:
        """
        Call an RPC method (async).

        Args:
            method: Method name to call
            params: Request data as bytes

        Returns:
            Response data as bytes

        Raises:
            TimeoutError: If request times out
            ConnectionError: If connection is lost
            RpcError: For other RPC errors
        """
        ...

    async def call_with_timeout(
        self, method: str, params: bytes, timeout_secs: float
    ) -> bytes:
        """
        Call an RPC method with custom timeout.

        Args:
            method: Method name to call
            params: Request data as bytes
            timeout_secs: Timeout in seconds

        Returns:
            Response data as bytes

        Raises:
            TimeoutError: If request times out
            ConnectionError: If connection is lost
            RpcError: For other RPC errors
        """
        ...

    async def call_server_streaming(
        self, method: str, params: bytes
    ) -> "AsyncStream":
        """
        Call a server streaming RPC method (one request, multiple responses).

        Args:
            method: Method name to call
            params: Request data as bytes

        Returns:
            Async stream of response bytes

        Raises:
            TimeoutError: If request times out
            ConnectionError: If connection is lost
            RpcError: For other RPC errors
        """
        ...

    async def call_client_streaming(
        self, method: str, request_list: List[bytes]
    ) -> bytes:
        """
        Call a client streaming RPC method (multiple requests, one response).

        Args:
            method: Method name to call
            request_list: List of request data as bytes

        Returns:
            Single response data as bytes

        Raises:
            TimeoutError: If request times out
            ConnectionError: If connection is lost
            RpcError: For other RPC errors
        """
        ...

    async def call_streaming(
        self, method: str, request_list: List[bytes]
    ) -> "AsyncStream":
        """
        Call a bidirectional streaming RPC method (multiple ↔ multiple).

        Args:
            method: Method name to call
            request_list: List of request data as bytes

        Returns:
            Async stream of response bytes

        Raises:
            TimeoutError: If request times out
            ConnectionError: If connection is lost
            RpcError: For other RPC errors
        """
        ...

# Server
class RpcServer:
    """Async RPC server for handling requests over QUIC+TLS."""

    def __init__(self, config: RpcConfig) -> None:
        """
        Create an RPC server.

        Args:
            config: RPC configuration with TLS settings and bind address
        """
        ...

    async def register(
        self,
        method_name: str,
        handler: Any,  # Callable[[bytes], Awaitable[bytes]]
    ) -> None:
        """
        Register an RPC method handler.

        Args:
            method_name: Name of the RPC method
            handler: Async function that takes bytes and returns bytes
        """
        ...

    async def serve(self) -> None:
        """
        Start serving requests (blocks until shutdown).

        Raises:
            TlsError: If TLS setup fails
            ConnectionError: If bind fails
        """
        ...

# Streaming
class AsyncStream:
    """
    Async iterator for streaming RPC responses.

    Use with async for:
        async for data in stream:
            process(data)
    """

    def __aiter__(self) -> "AsyncStream":
        """Return self as async iterator."""
        ...

    async def __anext__(self) -> bytes:
        """
        Get next item from stream.

        Returns:
            Next response data as bytes

        Raises:
            StopAsyncIteration: When stream ends
            RpcError: On stream errors
        """
        ...

    async def collect(self) -> List[bytes]:
        """
        Collect all items from stream into a list.

        Note: This loads all items into memory.

        Returns:
            List of all response data

        Raises:
            RpcError: On stream errors
        """
        ...

# Serialization functions (MessagePack for Python interop)
def python_to_msgpack_py(obj: Any) -> bytes:
    """
    Convert Python object to MessagePack bytes.

    Args:
        obj: Python dict or value to serialize

    Returns:
        Serialized bytes

    Raises:
        SerializationError: If serialization fails
    """
    ...

def msgpack_to_python_py(data: bytes) -> Any:
    """
    Convert MessagePack bytes to Python object.

    Args:
        data: Serialized bytes

    Returns:
        Deserialized Python dict or value

    Raises:
        SerializationError: If deserialization fails
    """
    ...

# Legacy bincode functions (deprecated, use MessagePack for Python)
def python_to_bincode_py(obj: Any) -> bytes:
    """
    [DEPRECATED] Convert Python object to bincode bytes.

    Use python_to_msgpack_py() instead for better Python compatibility.

    Args:
        obj: Python dict or value to serialize

    Returns:
        Serialized bytes

    Raises:
        SerializationError: If serialization fails
    """
    ...

def bincode_to_python_py(data: bytes) -> Any:
    """
    [DEPRECATED] Convert bincode bytes to Python object.

    Use msgpack_to_python_py() instead for better Python compatibility.

    Args:
        data: Serialized bytes

    Returns:
        Deserialized Python dict or value

    Raises:
        SerializationError: If deserialization fails
    """
    ...

# Exception hierarchy
class RpcError(Exception):
    """Base exception for all RPC errors."""
    ...

class ConnectionError(RpcError):
    """Connection-related errors (connection failed, lost, etc.)."""
    ...

class TimeoutError(RpcError):
    """Request timeout errors."""
    ...

class SerializationError(RpcError):
    """Serialization/deserialization errors."""
    ...

class TlsError(RpcError):
    """TLS/encryption errors."""
    ...

class StreamError(RpcError):
    """Streaming-related errors."""
    ...

class HandlerError(RpcError):
    """Handler execution errors."""
    ...
