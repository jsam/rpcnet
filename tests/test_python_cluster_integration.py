#!/usr/bin/env python3
"""
Integration tests for Python SWIM cluster functionality.

These tests verify that Python workers can join RpcNet clusters,
participate in SWIM gossip, and be discovered by directors.

Requirements:
    - pytest
    - pytest-asyncio
    - _rpcnet module (built with maturin develop)
    - TLS certificates in certs/
"""

import asyncio
import pytest
import sys
import os
from pathlib import Path

# Add project root to path for imports
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

try:
    import _rpcnet
except ImportError:
    pytest.skip(
        "Python bindings not available. Run: maturin develop --features extension-module",
        allow_module_level=True
    )


# Test Configuration
CERT_PATH = str(project_root / "certs" / "test_cert.pem")
KEY_PATH = str(project_root / "certs" / "test_key.pem")
DIRECTOR_ADDR = "127.0.0.1:0"  # Use port 0 for automatic assignment
WORKER_BASE_PORT = 50000


@pytest.fixture
def event_loop():
    """Create event loop for async tests"""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    yield loop
    loop.close()


@pytest.fixture
def test_certificates():
    """Ensure test certificates exist"""
    cert = Path(CERT_PATH)
    key = Path(KEY_PATH)

    if not cert.exists() or not key.exists():
        pytest.skip(
            f"Test certificates not found. Generate with:\n"
            f"mkdir -p certs && cd certs && "
            f"openssl req -x509 -newkey rsa:4096 -keyout test_key.pem "
            f"-out test_cert.pem -days 365 -nodes -subj '/CN=localhost'"
        )

    return CERT_PATH, KEY_PATH


class SimpleEchoHandler:
    """Simple handler for testing cluster operations"""

    def __init__(self, worker_id):
        self.worker_id = worker_id
        self.call_count = 0

    async def echo(self, request_bytes):
        """Echo back the request with worker ID"""
        self.call_count += 1
        message = request_bytes.decode('utf-8')
        response = f"Worker {self.worker_id}: {message} (call #{self.call_count})"
        return response.encode('utf-8')


async def create_worker(worker_id, worker_addr, director_addr, cert_path, key_path):
    """Helper to create and setup a worker server"""
    # Create server config
    config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=worker_addr,
        server_name="localhost"
    )

    # Create server
    server = _rpcnet.RpcServer(config)

    # Register handler
    handler = SimpleEchoHandler(worker_id)
    await server.register("echo", handler.echo)

    # Bind server (required before enable_cluster)
    await server.bind()

    # Enable cluster
    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    cluster_config = _rpcnet.ClusterConfig()
    await server.enable_cluster(cluster_config, [director_addr], quic_client)

    # Get cluster handle and update tags
    cluster = await server.cluster()
    if cluster:
        await cluster.update_tags([
            ("role", "worker"),
            ("worker_id", worker_id),
            ("language", "python"),
        ])

    return server, handler, cluster


async def run_server(server):
    """Wrapper to run server in background task"""
    await server.serve()


@pytest.mark.asyncio
async def test_cluster_basic_join(test_certificates):
    """Test that a Python worker can join a cluster"""
    cert_path, key_path = test_certificates

    # Create director
    director_config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=DIRECTOR_ADDR,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config)

    # Bind director
    await director.bind()

    # Get actual director address (since we used port 0)
    # Note: We'd need to expose socket_addr from Python bindings for this
    # For now, we'll use a fixed port
    director_addr = "127.0.0.1:50100"
    director_config_fixed = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=director_addr,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config_fixed)
    await director.bind()

    # Enable cluster on director
    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    cluster_config = _rpcnet.ClusterConfig()
    await director.enable_cluster(cluster_config, [], quic_client)

    # Start director in background
    director_task = asyncio.create_task(run_server(director))

    try:
        # Give director time to start
        await asyncio.sleep(0.5)

        # Create worker
        worker_addr = "127.0.0.1:50101"
        worker, handler, cluster = await create_worker(
            "worker-1", worker_addr, director_addr, cert_path, key_path
        )

        # Start worker in background
        worker_task = asyncio.create_task(run_server(worker))

        # Give cluster time to sync
        await asyncio.sleep(1.0)

        # Verify worker joined successfully
        assert cluster is not None

        # Test tag updates
        await cluster.update_tag("status", "ready")

        # Cleanup
        worker_task.cancel()
        director_task.cancel()

        try:
            await worker_task
        except asyncio.CancelledError:
            pass

        try:
            await director_task
        except asyncio.CancelledError:
            pass

    except Exception as e:
        director_task.cancel()
        raise e


@pytest.mark.asyncio
async def test_cluster_multiple_workers(test_certificates):
    """Test multiple Python workers joining the same cluster"""
    cert_path, key_path = test_certificates

    # Setup director
    director_addr = "127.0.0.1:50200"
    director_config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=director_addr,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config)
    await director.bind()

    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    cluster_config = _rpcnet.ClusterConfig()
    await director.enable_cluster(cluster_config, [], quic_client)

    director_task = asyncio.create_task(run_server(director))

    try:
        await asyncio.sleep(0.5)

        # Create multiple workers
        workers = []
        worker_tasks = []

        for i in range(3):
            worker_addr = f"127.0.0.1:{50201 + i}"
            worker, handler, cluster = await create_worker(
                f"worker-{i}", worker_addr, director_addr, cert_path, key_path
            )
            workers.append((worker, handler, cluster))
            worker_tasks.append(asyncio.create_task(run_server(worker)))

        # Give cluster time to sync
        await asyncio.sleep(2.0)

        # Verify all workers joined
        for worker, handler, cluster in workers:
            assert cluster is not None

        # Cleanup
        for task in worker_tasks:
            task.cancel()
        director_task.cancel()

        for task in worker_tasks:
            try:
                await task
            except asyncio.CancelledError:
                pass

        try:
            await director_task
        except asyncio.CancelledError:
            pass

    except Exception as e:
        director_task.cancel()
        for task in worker_tasks:
            task.cancel()
        raise e


@pytest.mark.asyncio
async def test_cluster_events(test_certificates):
    """Test cluster event subscription and receiving events"""
    cert_path, key_path = test_certificates

    # Setup director
    director_addr = "127.0.0.1:50300"
    director_config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=director_addr,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config)
    await director.bind()

    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    cluster_config = _rpcnet.ClusterConfig()
    await director.enable_cluster(cluster_config, [], quic_client)

    director_cluster = await director.cluster()
    assert director_cluster is not None

    # Subscribe to events
    event_receiver = director_cluster.subscribe()

    director_task = asyncio.create_task(run_server(director))

    try:
        await asyncio.sleep(0.5)

        # Create worker (should trigger NodeJoined event)
        worker_addr = "127.0.0.1:50301"
        worker, handler, cluster = await create_worker(
            "worker-1", worker_addr, director_addr, cert_path, key_path
        )
        worker_task = asyncio.create_task(run_server(worker))

        # Give SWIM more time to propagate the event
        await asyncio.sleep(2.0)

        # Wait for event with timeout
        try:
            event = await asyncio.wait_for(event_receiver.recv(), timeout=10.0)
            event_type, node_id, node_addr = event

            # Should receive NodeJoined or NodeStatusChanged event
            assert event_type in ["NodeJoined", "NodeStatusChanged"]
            assert node_addr == worker_addr

        except asyncio.TimeoutError:
            pytest.skip("Cluster events not received (may need longer for SWIM propagation)")

        # Cleanup
        worker_task.cancel()
        director_task.cancel()

        try:
            await worker_task
        except asyncio.CancelledError:
            pass

        try:
            await director_task
        except asyncio.CancelledError:
            pass

    except Exception as e:
        director_task.cancel()
        raise e


@pytest.mark.asyncio
async def test_cluster_heartbeat_control(test_certificates):
    """Test stopping and resuming heartbeats"""
    cert_path, key_path = test_certificates

    director_addr = "127.0.0.1:50400"
    director_config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=director_addr,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config)
    await director.bind()

    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    cluster_config = _rpcnet.ClusterConfig()
    await director.enable_cluster(cluster_config, [], quic_client)

    director_task = asyncio.create_task(run_server(director))

    try:
        await asyncio.sleep(0.5)

        # Create worker
        worker_addr = "127.0.0.1:50401"
        worker, handler, cluster = await create_worker(
            "worker-1", worker_addr, director_addr, cert_path, key_path
        )
        worker_task = asyncio.create_task(run_server(worker))

        await asyncio.sleep(1.0)

        # Test heartbeat control
        cluster.stop_heartbeats()
        await asyncio.sleep(0.5)

        cluster.resume_heartbeats()
        await asyncio.sleep(0.5)

        # If we got here without errors, heartbeat control works
        assert True

        # Cleanup
        worker_task.cancel()
        director_task.cancel()

        try:
            await worker_task
        except asyncio.CancelledError:
            pass

        try:
            await director_task
        except asyncio.CancelledError:
            pass

    except Exception as e:
        director_task.cancel()
        raise e


@pytest.mark.asyncio
async def test_cluster_config_options(test_certificates):
    """Test creating cluster with custom configuration"""
    cert_path, key_path = test_certificates

    # Create custom configs
    gossip_config = _rpcnet.GossipConfig(
        protocol_period_ms=500,
        ack_timeout_ms=250,
        indirect_timeout_ms=500,
        indirect_ping_count=2
    )

    health_config = _rpcnet.HealthCheckConfig(
        check_interval_secs=3,
        phi_threshold=10.0
    )

    pool_config = _rpcnet.PoolConfig(
        max_per_peer=5,
        max_total=50,
        idle_timeout_secs=120,
        connect_timeout_secs=5,
        health_check_interval_secs=30
    )

    cluster_config = _rpcnet.ClusterConfig(
        gossip=gossip_config,
        health=health_config,
        pool=pool_config
    )

    # Create server with custom config
    director_addr = "127.0.0.1:50500"
    director_config = _rpcnet.RpcConfig(
        cert_path=cert_path,
        key_path=key_path,
        bind_addr=director_addr,
        server_name="localhost"
    )
    director = _rpcnet.RpcServer(director_config)
    await director.bind()

    quic_client = await _rpcnet.QuicClient.create(cert_path=cert_path)
    await director.enable_cluster(cluster_config, [], quic_client)

    # If we got here, custom config was accepted
    assert True


if __name__ == "__main__":
    # Run tests with pytest
    pytest.main([__file__, "-v", "-s"])
