"""Generated DirectorRegistry server"""
import asyncio
from abc import ABC, abstractmethod
from typing import Optional
import _rpcnet
from .types import *

class DirectorRegistryHandler(ABC):
    """Handler interface for DirectorRegistry service

    Implement this class to define your service logic.
    All methods are async and should handle the business logic.
    """

    @abstractmethod
    async def get_worker(self, request: GetWorkerRequest) -> GetWorkerResponse:
        """Handle get_worker request"""
        pass



class DirectorRegistryServer:
    """RPC server for DirectorRegistry service

    This server wraps the low-level _rpcnet.RpcServer and
    automatically registers all handler methods.
    """

    def __init__(self, handler: DirectorRegistryHandler, config: _rpcnet.RpcConfig):
        """Initialize server with handler and configuration

        Args:
            handler: Implementation of DirectorRegistryHandler
            config: RPC configuration with TLS settings
        """
        self.handler = handler
        self.server = _rpcnet.RpcServer(config)

    async def _register_handlers(self):
        """Register all RPC method handlers"""
        
        async def handle_get_worker(request_bytes: bytes) -> bytes:
            # Deserialize request from MessagePack
            request_dict = _rpcnet.bincode_to_python_py(request_bytes)
            request = GetWorkerRequest(**request_dict)
            
            # Call handler
            response = await self.handler.get_worker(request)
            
            # Serialize response to MessagePack
            response_dict = response.__dict__
            return _rpcnet.python_to_bincode_py(response_dict)
        
        await self.server.register('get_worker', handle_get_worker)

    async def serve(self):
        """Start serving requests (blocks until shutdown)"""
        await self._register_handlers()
        await self.server.serve()
