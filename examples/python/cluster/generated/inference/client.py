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

    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        """Call infer RPC method"""
        # Serialize request to MessagePack bytes
        request_dict = request.__dict__
        request_bytes = _rpcnet.python_to_msgpack_py(request_dict)
        
        # Call RPC method 'Inference.infer'
        response_bytes = await self._client.call('Inference.infer', request_bytes)
        
        # Deserialize response from MessagePack
        response_dict = _rpcnet.msgpack_to_python_py(response_bytes)
        return deserialize_inferenceresponse(response_dict)

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
            
            # Unwrap Result if present (Rust streaming methods return Result<T, E>)
            if isinstance(response_dict, dict) and 'Ok' in response_dict:
                response_dict = response_dict['Ok']
            elif isinstance(response_dict, dict) and 'Err' in response_dict:
                # Handle error variant - could raise exception or yield error
                error_dict = response_dict['Err']
                raise Exception(f"RPC error: {error_dict}")
            
            yield deserialize_inferenceresponse(response_dict)

