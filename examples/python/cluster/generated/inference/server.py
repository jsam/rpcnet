"""Generated Inference server"""
import asyncio
from abc import ABC, abstractmethod
from typing import Optional
import _rpcnet
from .types import *

class InferenceHandler(ABC):
    """Handler interface for Inference service

    Implement this class to define your service logic.
    All methods are async and should handle the business logic.
    """

    @abstractmethod
    async def infer(self, request: InferenceRequest) -> InferenceResponse:
        """Handle infer request"""
        pass



class InferenceServer:
    """RPC server for Inference service

    This server wraps the low-level _rpcnet.RpcServer and
    automatically registers all handler methods.
    """

    def __init__(self, handler: InferenceHandler, config: _rpcnet.RpcConfig):
        """Initialize server with handler and configuration

        Args:
            handler: Implementation of InferenceHandler
            config: RPC configuration with TLS settings
        """
        self.handler = handler
        self.server = _rpcnet.RpcServer(config)

    async def _register_handlers(self):
        """Register all RPC method handlers"""
        
        async def handle_infer(request_bytes: bytes) -> bytes:
            # Deserialize request from MessagePack
            request_dict = _rpcnet.msgpack_to_python_py(request_bytes)
            request = InferenceRequest(**request_dict)
            
            # Call handler
            response = await self.handler.infer(request)
            
            # Serialize response to MessagePack
            response_dict = response.__dict__
            return _rpcnet.python_to_msgpack_py(response_dict)
        
        await self.server.register('infer', handle_infer)

    async def serve(self):
        """Start serving requests (blocks until shutdown)"""
        await self._register_handlers()
        await self.server.serve()
