# RpcNet Tests

This directory contains tests for RpcNet, including both Rust and Python integration tests.

## Test Structure

- `*.rs` - Rust unit and integration tests (run with `cargo test`)
- `test_python_cluster_integration.py` - Python cluster integration tests (run with `pytest`)
- `requirements-test.txt` - Python test dependencies

## Running Tests

### Rust Tests

```bash
# Run all Rust tests
cargo test

# Run specific test file
cargo test --test integration_tests

# Run with logging
RUST_LOG=debug cargo test

# Run tests for specific feature
cargo test --features python
```

### Python Tests

#### Prerequisites

1. **Build Python extension module:**
   ```bash
   maturin develop --features extension-module
   ```

2. **Generate TLS certificates** (if not already done):
   ```bash
   mkdir -p certs
   cd certs
   openssl req -x509 -newkey rsa:4096 \
     -keyout test_key.pem -out test_cert.pem \
     -days 365 -nodes -subj "/CN=localhost"
   cd ..
   ```

3. **Install test dependencies:**
   ```bash
   pip install -r tests/requirements-test.txt
   ```

#### Running Python Tests

```bash
# Run all Python tests with pytest
pytest tests/test_python_cluster_integration.py -v

# Run specific test
pytest tests/test_python_cluster_integration.py::test_cluster_basic_join -v

# Run with output
pytest tests/test_python_cluster_integration.py -v -s

# Run tests in parallel (if pytest-xdist installed)
pytest tests/test_python_cluster_integration.py -n auto
```

## Python Cluster Integration Tests

The `test_python_cluster_integration.py` file contains comprehensive tests for:

### Test Coverage

1. **`test_cluster_basic_join`**
   - Tests a single Python worker joining a cluster
   - Verifies cluster handle creation
   - Tests tag updates

2. **`test_cluster_multiple_workers`**
   - Tests multiple Python workers joining simultaneously
   - Verifies all workers can coexist in the cluster
   - Tests distributed worker discovery

3. **`test_cluster_events`**
   - Tests cluster event subscription
   - Verifies NodeJoined events are received
   - Tests event receiver functionality

4. **`test_cluster_heartbeat_control`**
   - Tests stopping heartbeats
   - Tests resuming heartbeats
   - Verifies heartbeat control doesn't break cluster

5. **`test_cluster_config_options`**
   - Tests custom GossipConfig
   - Tests custom HealthCheckConfig
   - Tests custom PoolConfig
   - Verifies configurations are accepted

### Test Implementation Details

Each test:
- Creates isolated clusters on unique ports (50000+ range)
- Sets up director and worker servers
- Performs cluster operations
- Cleans up resources properly (cancels tasks)
- Uses timeouts to prevent hanging

### Common Test Patterns

**Creating a Director:**
```python
director_config = _rpcnet.RpcConfig(
    cert_path=CERT_PATH,
    key_path=KEY_PATH,
    bind_addr="127.0.0.1:50000",
    server_name="localhost"
)
director = _rpcnet.RpcServer(director_config)
await director.bind()

quic_client = await _rpcnet.QuicClient.create(cert_path=CERT_PATH)
cluster_config = _rpcnet.ClusterConfig()
await director.enable_cluster(cluster_config, [], quic_client)

director_task = asyncio.create_task(director.serve())
```

**Creating a Worker:**
```python
worker_config = _rpcnet.RpcConfig(
    cert_path=CERT_PATH,
    key_path=KEY_PATH,
    bind_addr="127.0.0.1:50001",
    server_name="localhost"
)
worker = _rpcnet.RpcServer(worker_config)
await worker.register("echo", echo_handler)
await worker.bind()

quic_client = await _rpcnet.QuicClient.create(cert_path=CERT_PATH)
cluster_config = _rpcnet.ClusterConfig()
await worker.enable_cluster(cluster_config, ["127.0.0.1:50000"], quic_client)

cluster = await worker.cluster()
await cluster.update_tags([("role", "worker")])

worker_task = asyncio.create_task(worker.serve())
```

**Cleanup:**
```python
# Cancel tasks
director_task.cancel()
worker_task.cancel()

# Wait for cancellation
try:
    await director_task
except asyncio.CancelledError:
    pass
```

## Troubleshooting

### "Address already in use"
- Tests use ports in the 50000+ range
- If tests fail, check for hanging processes:
  ```bash
  lsof -i :50000-50500 | grep LISTEN
  ```
- Kill processes if needed:
  ```bash
  lsof -ti :50000-50500 | xargs kill -9
  ```

### "_rpcnet module not found"
- Build the Python extension:
  ```bash
  maturin develop --features extension-module
  ```

### "Test certificates not found"
- Generate certificates (see Prerequisites above)

### Tests hang or timeout
- Check if previous test processes are still running
- Increase timeouts in tests if needed
- Run tests with `-s` flag to see output

## CI/CD Integration

To run Python tests in CI:

```yaml
- name: Build Python Extension
  run: maturin develop --features extension-module

- name: Generate Test Certificates
  run: |
    mkdir -p certs
    openssl req -x509 -newkey rsa:4096 \
      -keyout certs/test_key.pem -out certs/test_cert.pem \
      -days 365 -nodes -subj "/CN=localhost"

- name: Install Test Dependencies
  run: pip install -r tests/requirements-test.txt

- name: Run Python Integration Tests
  run: pytest tests/test_python_cluster_integration.py -v
```

## Contributing Tests

When adding new Python cluster features:

1. Add corresponding tests to `test_python_cluster_integration.py`
2. Follow existing test patterns (setup, test, cleanup)
3. Use unique ports for each test to avoid conflicts
4. Add cleanup logic to prevent resource leaks
5. Document what the test verifies

## Performance Notes

- Tests use localhost networking (fast)
- Each test takes 1-5 seconds depending on cluster sync time
- Running all tests takes ~30 seconds
- Tests can be run in parallel with `pytest-xdist`
