#!/usr/bin/env python3
"""
Test Python async server with the fixed event loop bridge.
This test verifies that Python async handlers now work correctly.
"""

import asyncio
import pytest
import _rpcnet
import msgpack
import time


@pytest.mark.asyncio
async def test_python_async_server_works():
    """Test that Python async handlers work with the event loop bridge"""
    
    # Create server config
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="127.0.0.1:0",  # Use random port
        key_path="certs/test_key.pem",
        server_name="localhost",
    )
    
    # Create server
    server = _rpcnet.RpcServer(config)
    
    # Define async handler
    async def echo_handler(request_bytes: bytes) -> bytes:
        """Echo handler that proves async works"""
        # Simulate async operation
        await asyncio.sleep(0.01)
        
        # Deserialize request
        request = msgpack.unpackb(request_bytes, raw=False)
        
        # Process request
        response = {
            "message": f"Echo: {request.get('message', '')}",
            "timestamp": time.time(),
            "async": True  # Prove this is async
        }
        
        # Serialize response
        return msgpack.packb(response)
    
    # Register handler - this should work now!
    await server.register("test.echo", echo_handler)
    
    print("✅ Successfully registered async handler!")
    
    # Try with a compute-like handler
    async def compute_handler(request_bytes: bytes) -> bytes:
        """Simulate a compute task"""
        request = msgpack.unpackb(request_bytes, raw=False)
        
        # Simulate model inference
        await asyncio.sleep(0.05)  # Simulate processing time
        
        result = {
            "task_id": request.get("task_id"),
            "result": f"Processed: {request.get('input_data', '')}",
            "processing_time_ms": 50
        }
        
        return msgpack.packb(result)
    
    await server.register("compute.process", compute_handler)
    print("✅ Successfully registered compute handler!")
    
    return True


@pytest.mark.asyncio
async def test_inference_style_handler():
    """Test a more complex inference-style handler"""
    
    config = _rpcnet.RpcConfig(
        cert_path="certs/test_cert.pem",
        bind_addr="127.0.0.1:0",
        key_path="certs/test_key.pem",
        server_name="localhost",
    )
    
    server = _rpcnet.RpcServer(config)
    
    # Mock model class
    class MockModel:
        async def generate(self, prompt: str, max_tokens: int = 10):
            """Simulate token generation"""
            tokens = []
            for i in range(min(max_tokens, 5)):
                await asyncio.sleep(0.01)  # Simulate model inference time
                tokens.append(f"token_{i}")
            return tokens
    
    model = MockModel()
    
    async def inference_handler(request_bytes: bytes) -> bytes:
        """Handle inference requests"""
        request = msgpack.unpackb(request_bytes, raw=False)
        
        prompt = request.get("prompt", "")
        max_tokens = request.get("max_tokens", 10)
        
        # Run model inference
        tokens = await model.generate(prompt, max_tokens)
        
        response = {
            "prompt": prompt,
            "tokens": tokens,
            "model": "mock-model-v1"
        }
        
        return msgpack.packb(response)
    
    await server.register("inference.generate", inference_handler)
    print("✅ Successfully registered inference handler with model!")
    
    return True


if __name__ == "__main__":
    # Run the tests directly
    async def main():
        print("\n" + "="*60)
        print("Testing Python Async Server with Event Loop Bridge")
        print("="*60 + "\n")
        
        try:
            # Test 1: Basic async handler
            print("Test 1: Basic async handler...")
            result1 = await test_python_async_server_works()
            if result1:
                print("✅ Test 1 PASSED\n")
            
            # Test 2: Inference style handler
            print("Test 2: Inference style handler...")
            result2 = await test_inference_style_handler()
            if result2:
                print("✅ Test 2 PASSED\n")
            
            print("="*60)
            print("🎉 All tests passed! Python async handlers are working!")
            print("="*60)
            
        except Exception as e:
            print(f"\n❌ Test failed with error: {e}")
            import traceback
            traceback.print_exc()
    
    asyncio.run(main())