"""Generated Registry client"""
import asyncio
from typing import Optional, AsyncIterable, AsyncIterator
import _rpcnet
from .types import *

class RegistryClient:
    """Type-safe client for Registry service

    All methods are async and use the underlying _rpcnet.RpcClient
    for communication over QUIC+TLS.
    """

    def __init__(self, client: _rpcnet.RpcClient):
        self._client = client

    @staticmethod
    async def connect(
        addr: str,
        cert_path: str,
        key_path: Optional[str] = None,
        server_name: Optional[str] = None,
        timeout_secs: Optional[int] = None,
    ) -> 'RegistryClient':
        """Connect to Registry server

        Args:
            addr: Server address (e.g., '127.0.0.1:8080')
            cert_path: Path to TLS certificate
            key_path: Optional path to private key
            server_name: Optional server name for TLS
            timeout_secs: Optional timeout in seconds

        Returns:
            RegistryClient: Connected client instance
        """
        config = _rpcnet.RpcConfig(
            cert_path=cert_path,
            bind_addr='0.0.0.0:0',
            key_path=key_path,
            server_name=server_name,
            timeout_secs=timeout_secs,
        )
        client = await _rpcnet.RpcClient.connect(addr, config)
        return RegistryClient(client)

    async def get_worker(self, request: GetWorkerRequest) -> GetWorkerResponse:
        """Call get_worker RPC method"""
        # Serialize request to MessagePack bytes
        request_dict = request.__dict__
        request_bytes = _rpcnet.python_to_msgpack_py(request_dict)
        
        # Call RPC method 'Registry.get_worker'
        response_bytes = await self._client.call('Registry.get_worker', request_bytes)
        
        # Deserialize response from MessagePack
        response_dict = _rpcnet.msgpack_to_python_py(response_bytes)
        return GetWorkerResponse(**response_dict)

