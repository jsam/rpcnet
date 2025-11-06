"""Generated Inference client"""
import asyncio
from typing import Optional, AsyncIterable, AsyncIterator
import _rpcnet
from .types import *

class InferenceClient:
    """Type-safe client for Inference service

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
    ) -> 'InferenceClient':
        """Connect to Inference server

        Args:
            addr: Server address (e.g., '127.0.0.1:8080')
            cert_path: Path to TLS certificate
            key_path: Optional path to private key
            server_name: Optional server name for TLS
            timeout_secs: Optional timeout in seconds

        Returns:
            InferenceClient: Connected client instance
        """
        config = _rpcnet.RpcConfig(
            cert_path=cert_path,
            bind_addr='0.0.0.0:0',
            key_path=key_path,
            server_name=server_name,
            timeout_secs=timeout_secs,
        )
        client = await _rpcnet.RpcClient.connect(addr, config)
        return InferenceClient(client)

    async def generate(self, request_stream: AsyncIterable[InferenceRequest]) -> AsyncIterator[InferenceResponse]:
        """Streaming RPC method: generate"""
        # Collect and serialize request stream items
        request_list = []
        async for request in request_stream:
            request_dict = request.__dict__
            request_bytes = _rpcnet.python_to_msgpack_py(request_dict)
            request_list.append(request_bytes)
        
        # Call streaming RPC method 'Inference.generate'
        response_stream = await self._client.call_streaming('Inference.generate', request_list)
        
        # Yield deserialized responses
        async for response_bytes in response_stream:
            response_dict = _rpcnet.msgpack_to_python_py(response_bytes)
            yield InferenceResponse(**response_dict)

