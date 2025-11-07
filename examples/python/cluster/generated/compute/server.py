"""Generated Compute server"""
import asyncio
from abc import ABC, abstractmethod
from typing import Optional
import _rpcnet
from .types import *

class ComputeHandler(ABC):
    """Handler interface for Compute service

    Implement this class to define your service logic.
    All methods are async and should handle the business logic.
    """

    @abstractmethod
    async def process(self, request: ComputeRequest) -> ComputeResponse:
        """Handle process request"""
        pass



class ComputeServer:
    """RPC server for Compute service

    This server wraps the low-level _rpcnet.RpcServer and
    automatically registers all handler methods.
    """

    def __init__(self, handler: ComputeHandler, config: _rpcnet.RpcConfig):
        """Initialize server with handler and configuration

        Args:
            handler: Implementation of ComputeHandler
            config: RPC configuration with TLS settings
        """
        self.handler = handler
        self.server = _rpcnet.RpcServer(config)

    async def _register_handlers(self):
        """Register all RPC method handlers"""
        
        async def handle_process(request_bytes: bytes) -> bytes:
            # Deserialize request from MessagePack
            request_dict = _rpcnet.bincode_to_python_py(request_bytes)
            request = ComputeRequest(**request_dict)
            
            # Call handler
            response = await self.handler.process(request)
            
            # Serialize response to MessagePack
            response_dict = response.__dict__
            return _rpcnet.python_to_bincode_py(response_dict)
        
        await self.server.register('process', handle_process)

    async def serve(self):
        """Start serving requests (blocks until shutdown)"""
        await self._register_handlers()
        await self.server.serve()
