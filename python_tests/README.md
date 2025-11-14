# Python Bindings Test Suite

This directory contains the test suite for RpcNet's Python bindings.

## Prerequisites

- Python 3.8 or higher
- pytest and pytest-asyncio
- OpenSSL (for generating test certificates)
- maturin or cargo (for building the module)

Install Python dependencies:
```bash
pip install pytest pytest-asyncio maturin
```

## Running Tests

### Quick Start

Use the provided test runners:

```bash
# Using Python runner (recommended)
python python_tests/run_tests.py

# Using shell script
./python_tests/run_tests.sh
```

Both runners will:
1. Check that prerequisites are installed
2. Generate test certificates if needed
3. Build the Python module
4. Run the entire test suite

### Manual Testing

If you prefer to run tests manually:

```bash
# 1. Generate certificates (one time)
mkdir -p certs
cd certs
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
    -days 365 -nodes -subj "/CN=localhost"
cd ..

# 2. Build the module
maturin develop --features python

# 3. Run tests
pytest python_tests/ -v
```

## Test Files

- **`conftest.py`**: Pytest configuration and shared fixtures
  - Certificate path fixtures
  - Server and client fixtures
  - Async test configuration

- **`test_serialization.py`**: Unit tests for bincode serialization
  - Simple types (int, float, string, bool)
  - Complex types (dict, list, nested structures)
  - Edge cases (empty, large, unicode)
  - Error handling

- **`test_client.py`**: Integration tests for RPC client
  - Basic RPC calls
  - Timeout handling
  - Multiple concurrent calls
  - Large payloads
  - Multiple method handlers
  - Error conditions

- **`test_streaming.py`**: Tests for streaming functionality
  - Server streaming (one request → multiple responses)
  - Client streaming (multiple requests → one response)
  - Bidirectional streaming (multiple ↔ multiple)
  - Stream collection and early termination
  - Large data streaming
  - Error handling in streams

## Running Specific Tests

Run a specific test file:
```bash
pytest python_tests/test_serialization.py -v
```

Run a specific test:
```bash
pytest python_tests/test_client.py::test_basic_rpc_call -v
```

Run tests matching a pattern:
```bash
pytest python_tests/ -k "streaming" -v
```

## Test Options

Common pytest options:

```bash
# Verbose output
pytest python_tests/ -v

# Show local variables on failure
pytest python_tests/ -l

# Stop on first failure
pytest python_tests/ -x

# Run tests in parallel (requires pytest-xdist)
pytest python_tests/ -n auto

# Show print statements
pytest python_tests/ -s

# Generate coverage report (requires pytest-cov)
pytest python_tests/ --cov=_rpcnet --cov-report=html
```

## Async Testing

All async tests use `@pytest.mark.asyncio` and work with the `pytest-asyncio` plugin. The configuration is in `conftest.py`:

```python
pytest_plugins = ('pytest_asyncio',)
```

## Fixtures

### Certificate Fixtures
- `certs_dir`: Path to certificates directory
- `test_cert`: Path to test certificate file
- `test_key`: Path to test private key file

### Server/Client Fixtures
- `rpc_server`: Pre-configured test server with echo handler
- `rpc_client`: Pre-configured test client connected to server

Example usage:
```python
@pytest.mark.asyncio
async def test_my_feature(rpc_server, rpc_client):
    request = b"test data"
    response = await rpc_client.call("echo", request)
    assert response == request
```

## Troubleshooting

### Module Import Errors

If you see `ImportError: No module named '_rpcnet'`:
- Make sure you've run `maturin develop --features python`
- Check that you're in a virtual environment if using one
- Try `pip install -e .` as an alternative

### Certificate Errors

If you see TLS/certificate errors:
- Run the test runner which generates certificates automatically
- Or manually generate with the OpenSSL command above
- Certificates are valid for 365 days

### Timeout Errors

If tests timeout:
- Check that the server is starting properly
- Increase timeout values in tests if on a slow machine
- Make sure ports are not already in use

### AsyncIO Errors

If you see event loop errors:
- Make sure pytest-asyncio is installed
- Check that tests are marked with `@pytest.mark.asyncio`
- Use `pytest --asyncio-mode=auto` if needed

## CI/CD Integration

### GitHub Actions Example

```yaml
- name: Install Python dependencies
  run: pip install pytest pytest-asyncio maturin

- name: Run Python tests
  run: python python_tests/run_tests.py
```

### GitLab CI Example

```yaml
test:python:
  script:
    - pip install pytest pytest-asyncio maturin
    - python python_tests/run_tests.py
```

## Writing New Tests

When adding new tests:

1. **Unit tests** (test_serialization.py style):
   - No fixtures needed
   - Fast, isolated tests
   - Test one thing at a time

2. **Integration tests** (test_client.py style):
   - Use `rpc_server` and `rpc_client` fixtures
   - Test actual RPC communication
   - Mark with `@pytest.mark.asyncio`

3. **Streaming tests** (test_streaming.py style):
   - Set up custom handlers if needed
   - Test all streaming patterns
   - Verify cleanup with try/finally

Example template:
```python
@pytest.mark.asyncio
async def test_my_feature(test_cert, test_key):
    """Test description."""
    # Setup
    config = _rpcnet.RpcConfig(
        cert_path=test_cert,
        bind_addr="127.0.0.1:0",
        key_path=test_key,
    )
    server = _rpcnet.RpcServer(config)

    async def my_handler(request_bytes: bytes) -> bytes:
        # Handler logic
        return response_bytes

    await server.register("my_method", my_handler)

    server_task = asyncio.create_task(server.serve())
    await asyncio.sleep(0.1)  # Let server start

    try:
        # Test logic
        client = await _rpcnet.RpcClient.connect(...)
        result = await client.call("my_method", ...)
        assert result == expected
    finally:
        # Cleanup
        server_task.cancel()
        try:
            await server_task
        except asyncio.CancelledError:
            pass
```

## Performance Testing

For performance testing, consider:

1. Using `pytest-benchmark` for microbenchmarks
2. Testing with various payload sizes
3. Testing concurrent load (multiple simultaneous clients)
4. Profiling with `py-spy` or `austin` (see GIL_PROFILING_GUIDE.md)

## Notes

- Tests assume localhost networking is available
- Tests use random ports (bind_addr="127.0.0.1:0") to avoid conflicts
- Server tasks are properly cleaned up in finally blocks
- All tests should be idempotent and independent
