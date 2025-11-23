"""
RpcNet - Low-latency RPC library with QUIC+TLS and SWIM gossip protocol

This package provides Python bindings for the RpcNet library, offering:
- High-performance RPC client/server implementation
- QUIC+TLS transport for secure, low-latency communication
- SWIM gossip protocol for distributed systems
- Automatic code generation from service definitions

Example:
    import rpcnet
    
    # Create a client
    client = await rpcnet.RpcClient.connect("127.0.0.1:5000")
    
    # Make RPC calls
    response = await client.call("method_name", request_data)
"""

# Import the native extension
from ._rpcnet import *

# Version info
__version__ = "0.1.0"
__all__ = [
    # Core classes from the extension
    "AsyncStream",
    "BlockingClient",
    "Cluster",
    "ClusterConfig",
    "ClusterEventReceiver",
    "ConnectionError",
    "GossipConfig",
    "HealthCheckConfig",
    "PoolConfig",
    "QuicClient",
    "RpcClient",
    "RpcConfig",
    "RpcServer",
    "SerializationError",
    "TimeoutError",
    "TlsError",
    # Serialization utilities
    "bincode_to_python_py",
    "msgpack_to_python_py",
    "python_to_bincode_py",
    "python_to_msgpack_py",
]

def rpcnet_gen_cli():
    """Entry point for the rpcnet-gen CLI tool."""
    import os
    import sys
    import subprocess
    from pathlib import Path
    
    # Find the rpcnet-gen binary
    # It should be installed alongside the Python package
    package_dir = Path(__file__).parent
    
    # Try different possible locations
    possible_paths = [
        package_dir / "rpcnet-gen",  # Unix
        package_dir / "rpcnet-gen.exe",  # Windows
        package_dir.parent / "rpcnet-gen",
        package_dir.parent / "rpcnet-gen.exe",
    ]
    
    rpcnet_gen_path = None
    for path in possible_paths:
        if path.exists() and path.is_file():
            rpcnet_gen_path = path
            break
    
    if not rpcnet_gen_path:
        print("Error: rpcnet-gen binary not found in package", file=sys.stderr)
        print("Please ensure the package was installed correctly", file=sys.stderr)
        sys.exit(1)
    
    # Execute rpcnet-gen with the provided arguments
    try:
        result = subprocess.run(
            [str(rpcnet_gen_path)] + sys.argv[1:],
            check=False
        )
        sys.exit(result.returncode)
    except Exception as e:
        print(f"Error executing rpcnet-gen: {e}", file=sys.stderr)
        sys.exit(1)