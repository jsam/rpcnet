# Testing Guide for RpcNet

This document provides comprehensive information about testing the RpcNet library, including how to run tests, interpret coverage reports, and contribute new tests.

**Version**: 0.2.0  
**Last Updated**: 2025-01-06

## Table of Contents

- [Test Organization](#test-organization)
- [Running Tests](#running-tests)
- [Test Coverage](#test-coverage)
- [Test Categories](#test-categories)
- [Writing New Tests](#writing-new-tests)
- [Continuous Integration](#continuous-integration)
- [Troubleshooting](#troubleshooting)

## Test Organization

RpcNet uses a multi-layered testing approach:

### Directory Structure
```
rpcnet/
├── src/
│   ├── lib.rs              # Core library with embedded unit tests
│   ├── cluster/            # Cluster management with tests
│   ├── streaming/          # Streaming support
│   └── codegen/            # Code generation (always included)
├── tests/
│   ├── *_tests.rs          # Comprehensive test suites
│   ├── cluster_integration.rs # Cluster functionality tests
│   └── streaming_*.rs      # Streaming-specific tests
├── examples/
│   ├── basic_*/            # Basic examples
│   ├── cluster/            # Cluster example with full setup
│   └── */                  # Code generation examples
└── benches/                # Performance benchmarks (172K+ RPS)
```

### Test Types

1. **Unit Tests** (`src/lib.rs` and `tests/*_unit_tests.rs`)
   - Test individual components in isolation
   - Cover all public APIs
   - Test serialization/deserialization
   - Validate configuration builders

2. **Integration Tests** (`tests/*_integration_tests.rs`)
   - Test client-server communication
   - Test concurrent operations
   - Test large payload handling
   - Test real network scenarios

3. **Cluster Tests** (`tests/cluster_integration.rs`)
   - Test gossip protocol (SWIM)
   - Test failure detection (Phi Accrual)
   - Test load balancing strategies
   - Test worker discovery and health checking
   - Test connection pooling

4. **Streaming Tests** (`tests/streaming_*.rs`, `tests/*_streaming_*.rs`)
   - Test bidirectional streaming
   - Test client streaming
   - Test server streaming
   - Test backpressure handling
   - Test stream error recovery

5. **Error Scenario Tests** (`tests/*_error_tests.rs`)
   - Test error handling and recovery
   - Test network failures
   - Test configuration errors
   - Test timeout scenarios

6. **Code Generation Tests** (`tests/codegen_*.rs`)
   - Test generated client code
   - Test generated server code
   - Test type generation
   - Test edge cases in code generation

7. **Example Tests** (`examples/`)
   - Demonstrate real-world usage
   - Serve as integration tests
   - Show best practices
   - Include cluster example with failure scenarios

## Running Tests

### Basic Test Commands

```bash
# Run all tests
cargo test

# Run only unit tests (library tests)
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run specific test suite
cargo test --test cluster_integration
cargo test --test streaming_coverage_tests

# Run tests with output
cargo test -- --nocapture

# Run tests in single thread (for debugging)
cargo test -- --test-threads=1

# Run tests with all features
cargo test --all-features

# Run cluster example tests
cargo test --manifest-path examples/cluster/Cargo.toml
```

### Using Make Commands

We provide convenient Make targets:

```bash
# Run all tests
make test

# Run unit tests only
make test-unit

# Run integration tests only
make test-integration

# Test that examples compile and run
make test-examples
```

### Test Configuration

Tests use the certificates in `certs/` directory:
- `certs/test_cert.pem` - Test certificate
- `certs/test_key.pem` - Test private key

## Test Coverage

### Generating Coverage Reports

We use `cargo-tarpaulin` for code coverage analysis:

```bash
# Install tarpaulin (if not already installed)
cargo install cargo-tarpaulin

# Generate HTML coverage report
make coverage

# Check coverage meets 90% threshold
make coverage-check

# Analyze coverage gaps by priority
make coverage-gaps

# Generate coverage for CI/CD
make coverage-ci

# Use analysis scripts for detailed reports
./scripts/analyze-coverage.sh
./scripts/report-gaps.sh
```

### Coverage Requirements

RpcNet enforces strict coverage requirements:

- **Overall Project**: 90%+ line coverage (mandatory)
- **Security Features**: 95%+ coverage (critical)
- **Core RPC**: 95%+ coverage (critical)  
- **Transport Layer**: 90%+ coverage (high priority)
- **Cluster Management**: 90%+ coverage (high priority)
- **Streaming**: 90%+ coverage (high priority)
- **Code Generation**: 85%+ coverage (medium priority)
- **Utilities**: 75%+ coverage (low priority)

**Current Status**: 90%+ coverage maintained across all modules

### Coverage Reports

Coverage reports are generated in:
- `target/coverage/tarpaulin-report.html` - HTML report
- `target/coverage/cobertura.xml` - XML for CI systems

## Test Categories

### Unit Tests Coverage

#### RpcRequest Tests
- Construction with various parameters
- Accessor methods (`id()`, `method()`, `params()`)
- Serialization/deserialization
- Edge cases (empty params, large IDs, Unicode method names)

#### RpcResponse Tests  
- Success and error response construction
- Conversion from `Result<Vec<u8>, RpcError>`
- Accessor methods
- Serialization with various payload sizes

#### RpcConfig Tests
- Builder pattern functionality
- Path handling (String, &str, PathBuf)
- Network configuration options
- Keep-alive settings
- Configuration cloning

#### RpcError Tests
- Error type display formatting
- Error conversion from other types
- Debug formatting
- Error propagation

#### Cluster Component Tests
- **NodeRegistry**: Node tracking and filtering
- **WorkerRegistry**: Worker discovery and load balancing
- **HealthChecker**: Phi Accrual failure detection
- **GossipProtocol**: SWIM-based membership
- **ConnectionPool**: Connection reuse and pooling
- **PartitionDetector**: Network partition handling

### Integration Tests Coverage

#### Client-Server Communication
- Basic request/response cycles
- Multiple method registration
- Handler state management
- Connection reuse

#### Concurrent Operations
- Multiple clients accessing server
- Concurrent requests from single client
- Request ID handling
- Race condition prevention

#### Large Payload Handling
- Requests up to 1MB+
- Response payload testing
- Memory usage validation
- Performance with large data

#### Network Scenarios
- Connection failures
- Timeout handling
- Server shutdown during requests
- Certificate validation

#### Cluster Integration
- **Worker Discovery**: Automatic discovery via gossip
- **Load Balancing**: Round Robin, Random, Least Connections
- **Failure Detection**: Worker failure and recovery
- **Tag-Based Routing**: Filter workers by tags
- **Connection Pooling**: Pool creation and reuse
- **Partition Recovery**: Network split and rejoin
- **Health Monitoring**: Continuous health checks

#### Streaming Integration
- **Bidirectional Streaming**: Client and server streaming simultaneously
- **Client Streaming**: Multiple client messages, single server response
- **Server Streaming**: Single client request, multiple server responses
- **Error Handling**: Stream errors and recovery
- **Backpressure**: Flow control and buffering

### Error Scenario Tests

#### Network Errors
- Connection refused
- Invalid server addresses
- Certificate/key file issues
- TLS handshake failures

#### Handler Errors
- Custom error types
- Serialization failures
- Validation errors
- Timeout scenarios

#### Configuration Errors
- Missing certificate files
- Invalid bind addresses
- Missing key paths
- Invalid server names

## Writing New Tests

### Unit Test Guidelines

```rust
use rpcnet::{RpcClient, RpcConfig, RpcServer, RpcError};

#[test]
fn test_feature_name() {
    // Arrange
    let input = create_test_input();
    
    // Act
    let result = function_under_test(input);
    
    // Assert
    assert_eq!(result, expected_output);
}

#[tokio::test]
async fn test_async_feature() {
    // For async tests
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### Integration Test Guidelines

```rust
use rpcnet::*;
use std::time::Duration;

#[tokio::test]
async fn test_client_server_interaction() {
    // Create test configuration
    let config = test_config();
    
    // Set up server
    let mut server = RpcServer::new(config.clone());
    server.register("test_method", handler_function).await;
    
    // Start server
    let (addr, _handle) = start_test_server(server).await.unwrap();
    
    // Create client and test
    let client = RpcClient::connect(addr, config).await.unwrap();
    let response = client.call("test_method", test_data).await.unwrap();
    
    // Assertions
    assert_eq!(response, expected_response);
}
```

### Test Helpers

Common helper functions are available:

```rust
fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
}

async fn start_test_server(server: RpcServer) -> Result<(SocketAddr, JoinHandle), RpcError> {
    // Implementation provided in test modules
}
```

### Test Naming Conventions

- `test_[feature]_[scenario]` - Basic unit tests
- `test_[component]_[behavior]` - Component tests  
- `test_[error_type]_handling` - Error scenario tests
- `test_[feature]_edge_cases` - Edge case tests

### Assertions Guidelines

Prefer specific assertions:
```rust
// Good
assert_eq!(actual, expected);
assert!(matches!(result, Ok(value) if value > 0));

// Less preferred
assert!(condition);
```

## Continuous Integration

### GitHub Actions Integration

Tests run automatically on:
- Every pull request
- Pushes to main branch
- Release tags

### CI Test Commands

```bash
# Run all CI checks
make ci

# Individual CI components
make ci-test     # All tests
make ci-coverage # Coverage analysis
make ci-lint     # Code quality
```

### Coverage Requirements

CI enforces:
- **Minimum 90% line coverage** (blocks PRs if below threshold)
- All tests must pass
- No clippy warnings  
- Proper code formatting
- Coverage gap analysis and reporting

The CI will:
- Generate coverage reports on every PR
- Block merge if coverage drops below 90%
- Post coverage status as PR comment
- Upload reports to Codecov for tracking

## Troubleshooting

### Common Issues

#### Certificate Errors
```bash
# Ensure test certificates exist
ls -la certs/
# If missing, generate new test certificates
```

#### Timeout Issues
- Tests use shorter timeouts in test mode (2s vs 30s)
- Network tests may be flaky in CI environments
- Use `--test-threads=1` for debugging

#### Port Binding Errors
- Tests use ephemeral ports (`127.0.0.1:0`)
- Multiple test runs may conflict
- Clean up with `cargo clean`

#### Coverage Tool Issues
```bash
# Install/reinstall tarpaulin
cargo install cargo-tarpaulin --force

# Clear previous coverage data
rm -rf target/tarpaulin

# Run with verbose output
cargo tarpaulin --verbose
```

### Debug Commands

```bash
# Run tests with debug output
RUST_LOG=debug cargo test

# Run specific test with backtrace
RUST_BACKTRACE=1 cargo test test_name -- --exact

# Run tests without capturing output
cargo test -- --nocapture
```

### Performance Debugging

```bash
# Run with profiling
cargo test --release

# Use benchmark mode for performance tests
cargo bench

# Memory usage monitoring
valgrind --tool=memcheck cargo test (Linux only)
```

## Test Data Management

### Test Certificates
- Located in `certs/` directory
- Self-signed certificates for testing only
- **⚠️ NOT suitable for production use**
- Automatically trusted in test environment
- Generated via OpenSSL:
  ```bash
  openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \
    -out test_cert.pem -days 365 -nodes -subj "/CN=localhost"
  ```

### Test Data Cleanup
```bash
# Clean all build artifacts
make clean

# Clean only coverage data
rm -rf target/coverage target/tarpaulin
```

## Contributing Test Guidelines

When contributing new tests:

1. **Follow naming conventions** - Use descriptive, consistent names
2. **Test edge cases** - Don't just test happy paths
3. **Include error scenarios** - Test what happens when things go wrong
4. **Document complex tests** - Add comments for non-obvious test logic
5. **Keep tests isolated** - Each test should be independent
6. **Use appropriate test types** - Unit tests for components, integration for workflows

### Code Review Checklist

- [ ] All new code has corresponding tests
- [ ] Tests cover both success and error cases
- [ ] Tests are deterministic and don't rely on external state
- [ ] Test names clearly describe what is being tested
- [ ] Integration tests use proper async/await patterns
- [ ] Error scenarios are properly handled
- [ ] Tests run successfully in CI environment

## Running Cluster Tests

The cluster example includes comprehensive tests for distributed features:

```bash
# Run cluster integration tests
cargo test --test cluster_integration

# Run cluster example manually (for visual inspection)
# Terminal 1: Director
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin director

# Terminal 2: Worker A
WORKER_LABEL=worker-a WORKER_ADDR=127.0.0.1:62001 \
  DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 3: Worker B  
WORKER_LABEL=worker-b WORKER_ADDR=127.0.0.1:62002 \
  DIRECTOR_ADDR=127.0.0.1:61000 WORKER_FAILURE_ENABLED=true RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin worker

# Terminal 4: Client
DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \
  cargo run --manifest-path examples/cluster/Cargo.toml --bin client
```

See `examples/cluster/README.md` for detailed cluster testing scenarios.

## Performance Testing

```bash
# Run benchmarks
cargo bench

# Run benchmarks with performance optimizations
cargo bench --features perf

# Test specific benchmark
cargo bench --bench simple

# Current performance targets:
# - 172K+ requests/second with QUIC+TLS
# - Sub-millisecond latency (< 0.1ms overhead)
# - 10K+ concurrent streams per connection
```

---

For more information about RpcNet development, see:
- [README.md](README.md) - Project overview and features
- [DEVELOPER.md](DEVELOPER.md) - Development and publication guide
- [examples/cluster/README.md](examples/cluster/README.md) - Cluster setup guide
- [API Documentation](https://docs.rs/rpcnet) - Generated API docs
- [Crates.io](https://crates.io/crates/rpcnet) - Published package