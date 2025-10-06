# Zero-Downtime QUIC Connection Migration

This document describes the zero-downtime QUIC connection migration feature implemented in RpcNet, allowing live connections to be seamlessly transferred between backend workers without client reconnection.

## Overview

The migration system enables RpcNet applications to:

- **Seamlessly transfer active QUIC connections** between workers without dropping client connections
- **Preserve application state** during migration using encrypted snapshots
- **Maintain security** through cryptographic verification and token-based authentication
- **Handle failures gracefully** with automatic rollback and health monitoring
- **Scale horizontally** by redistributing connections across worker pools

## Architecture

### Core Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client        │    │   Director       │    │   Workers       │
│                 │    │                  │    │                 │
│ QUIC Connection │◄──►│ Connection Pool  │◄──►│ State Handlers  │
│ State Tracking  │    │ Migration Logic  │    │ Service Logic   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │
                              ▼
                       ┌──────────────────┐
                       │ Migration System │
                       │                  │
                       │ • State Capture  │
                       │ • Encryption     │ 
                       │ • Transfer       │
                       │ • Restoration    │
                       └──────────────────┘
```

### Migration Components

| Component | Purpose | Location |
|-----------|---------|----------|
| **Connection State Snapshot** | Captures connection and application state | `src/migration/models/connection_state_snapshot.rs` |
| **Migration Request/Confirmation** | Manages migration lifecycle | `src/migration/models/migration_*.rs` |
| **Encryption Service** | Secures state data with AES-256-GCM | `src/migration/state/encryption.rs` |
| **Serialization Service** | Handles state compression and chunking | `src/migration/state/serialization.rs` |
| **Token Manager** | Authentication and rate limiting | `src/migration/auth/token_manager.rs` |
| **State Machine** | Migration workflow orchestration | `src/migration/state_machine.rs` |
| **Session Manager** | Active connection tracking | `src/migration/session_manager.rs` |
| **Health Checker** | Migration status monitoring | `src/migration/health.rs` |

## Migration Workflow

### 1. State Capture

```rust
use rpcnet::migration::models::ConnectionStateSnapshot;

// Capture current connection state
let mut snapshot = ConnectionStateSnapshot::new(
    connection_id.clone(),
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
);

// Add stream buffers
snapshot = snapshot.add_stream_buffer(stream_id, buffer_data);

// Add application state
snapshot = snapshot.add_application_state("user_id".to_string(), user_id);
```

### 2. Encryption and Serialization

```rust
use rpcnet::migration::state::{
    encryption::{EncryptionService, EncryptionKey},
    serialization::{SerializationService, SerializationConfig, CompressionMethod},
};

// Initialize services
let encryption_service = EncryptionService::new();
let key = EncryptionKey::generate();
let serialization_service = SerializationService::new(SerializationConfig {
    compression: CompressionMethod::Gzip,
    chunk_size: 1024 * 1024, // 1MB chunks
});

// Serialize and encrypt
let serialized = serialization_service.serialize(&snapshot).await?;
let encrypted = encryption_service.encrypt(&serialized, &key)?;
```

### 3. Migration Request

```rust
use rpcnet::migration::models::MigrationRequest;
use rpcnet::migration::auth::token_manager::{MigrationTokenManager, TokenValidationConfig};

// Create migration request
let migration_request = MigrationRequest::new(
    "migration-001".to_string(),
    connection_id,
    source_worker_id,
    target_worker_id,
    encrypted_state,
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
);

// Generate secure token
let token_manager = MigrationTokenManager::new(TokenValidationConfig {
    max_requests_per_second: 100,
    token_expiry_seconds: 3600,
    secret_key: secret_key.clone(),
});

let token = token_manager.generate_token(&migration_request)?;
```

### 4. State Transfer and Restoration

```rust
// On target worker: validate and decrypt
let is_valid = token_manager.validate_token(&token, &migration_request)?;
assert!(is_valid);

let decrypted = encryption_service.decrypt(&encrypted_state, &key)?;
let restored_snapshot: ConnectionStateSnapshot = 
    serialization_service.deserialize(&decrypted).await?;

// Restore connection state
for (stream_id, buffer) in restored_snapshot.stream_buffers() {
    restore_stream_buffer(*stream_id, buffer.clone());
}

for (key, value) in restored_snapshot.application_state() {
    restore_application_state(key.clone(), value.clone());
}
```

### 5. Confirmation and Completion

```rust
use rpcnet::migration::models::{MigrationConfirmation, MigrationParty};

// Confirm successful migration
let confirmation = MigrationConfirmation::new(
    migration_request.migration_id().clone(),
    migration_request.connection_id().clone(),
    MigrationParty::System,
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
    true, // success
    Some("Migration completed successfully".to_string()),
);
```

## State Machine

The migration process follows a well-defined state machine:

```
    Idle
     │
     ▼
  Preparing ──► Failed
     │            ▲
     ▼            │
Transferring ─────┘
     │
     ▼
  Completed
```

### State Transitions

| From | To | Trigger | Description |
|------|----|---------| ------------|
| `Idle` | `Preparing` | Migration request | Begin migration preparation |
| `Preparing` | `Transferring` | State capture complete | Start state transfer |
| `Preparing` | `Failed` | Preparation error | Abort migration |
| `Transferring` | `Completed` | Successful confirmation | Migration complete |
| `Transferring` | `Failed` | Transfer error | Rollback migration |

## Security Features

### Encryption

- **Algorithm**: AES-256-GCM for authenticated encryption
- **Key Management**: Secure key generation and rotation
- **Integrity**: Built-in authentication prevents tampering

### Authentication

- **HMAC-SHA256**: Token-based request authentication
- **Rate Limiting**: Configurable request rate limits
- **Expiration**: Time-based token expiry

### Validation

- **State Integrity**: Cryptographic verification of state data
- **Request Validation**: Comprehensive input sanitization
- **Connection Verification**: Source and target worker validation

## Performance Characteristics

### Benchmarks

Based on the migration benchmarks (`benches/migration_benchmarks.rs`):

| Operation | 1KB State | 1MB State | 10MB State |
|-----------|-----------|-----------|------------|
| **Encryption** | ~10μs | ~1ms | ~10ms |
| **Serialization (Gzip)** | ~50μs | ~5ms | ~50ms |
| **Complete Workflow** | ~100μs | ~10ms | ~100ms |

### Memory Usage

- **Streaming**: Large states processed in configurable chunks
- **Compression**: Automatic compression reduces transfer size
- **Cleanup**: Automatic memory cleanup after migration

### Scalability

- **Concurrent Migrations**: Supports multiple simultaneous migrations
- **Rate Limiting**: Prevents resource exhaustion
- **Fault Tolerance**: Graceful handling of worker failures

## Configuration

### Encryption Configuration

```rust
// Configure encryption service
let encryption_service = EncryptionService::new();
let key = EncryptionKey::from_bytes(&[/* 32-byte key */])?;
```

### Serialization Configuration

```rust
let config = SerializationConfig {
    compression: CompressionMethod::Gzip, // None, Gzip, or Zstd
    chunk_size: 1024 * 1024, // 1MB chunks
};
```

### Token Manager Configuration

```rust
let config = TokenValidationConfig {
    max_requests_per_second: 100,
    token_expiry_seconds: 3600, // 1 hour
    secret_key: your_secret_key,
};
```

## Error Handling

### Migration Failures

The system handles various failure scenarios:

```rust
match migration_result {
    Ok(confirmation) => {
        // Migration successful
        log::info!("Migration completed: {}", confirmation.migration_id());
    }
    Err(MigrationError::InvalidToken) => {
        // Authentication failed
        log::error!("Migration failed: invalid token");
    }
    Err(MigrationError::StateCorruption) => {
        // State data corrupted
        log::error!("Migration failed: state corruption detected");
    }
    Err(MigrationError::TargetUnavailable) => {
        // Target worker unavailable
        log::error!("Migration failed: target worker unavailable");
    }
}
```

### Rollback Strategy

Failed migrations trigger automatic rollback:

1. **State Cleanup**: Remove partial state from target worker
2. **Connection Restoration**: Restore connection on source worker
3. **Health Check**: Verify connection health after rollback
4. **Notification**: Alert monitoring systems of failure

## Monitoring and Health Checks

### Health Checker

```rust
use rpcnet::migration::health::HealthChecker;

let health_checker = HealthChecker::new();
let health_status = health_checker.check_migration_health(&migration_request).await?;

if health_status.is_healthy() {
    println!("Migration system healthy");
} else {
    println!("Migration issues detected: {:?}", health_status.issues());
}
```

### Metrics

The migration system provides metrics for:

- **Migration Success Rate**: Percentage of successful migrations
- **Migration Latency**: Time to complete migrations
- **State Size Distribution**: Size of migrated states
- **Error Rates**: Types and frequency of migration errors

## Integration Guide

### Basic Integration

1. **Add Migration Support to Your Service**:

```rust
use rpcnet::migration::session_manager::SessionManager;

pub struct MyService {
    session_manager: SessionManager,
    // ... other fields
}

impl MyService {
    pub async fn handle_migration(&self, request: MigrationRequest) -> Result<(), MigrationError> {
        // Implement migration handling
    }
}
```

2. **Register Migration Handlers**:

```rust
// In your RPC service setup
service.register_migration_handler(|request| async move {
    // Handle incoming migration requests
});
```

3. **Implement State Capture**:

```rust
impl MyService {
    fn capture_connection_state(&self, connection_id: &str) -> ConnectionStateSnapshot {
        let mut snapshot = ConnectionStateSnapshot::new(
            connection_id.to_string(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
        );
        
        // Capture your service-specific state
        // snapshot = snapshot.add_application_state(key, value);
        
        snapshot
    }
}
```

### Advanced Integration

For advanced use cases, you can:

- **Custom State Serialization**: Implement custom serialization for complex state
- **Migration Policies**: Define when and how migrations should occur
- **Load Balancing Integration**: Use migration for dynamic load balancing
- **Monitoring Integration**: Connect to your monitoring infrastructure

## Example: Connection Swap Demo

The `examples/connection_swap/` directory contains a complete working example:

```bash
# Terminal 1: Start director
CONNECTION_SWAP_DIRECTOR_ADDR=127.0.0.1:61000 \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin director

# Terminal 2: Start worker A
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 3: Start worker B  
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
CONNECTION_SWAP_WORKER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker

# Terminal 4: Start client
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
RUST_LOG=info \
cargo run --manifest-path examples/connection_swap/Cargo.toml --bin client
```

This example demonstrates:
- Live connection transfer between workers
- Seamless client experience (no reconnection)
- Failure simulation and recovery
- State preservation across migrations

## Testing

### Unit Tests

Run individual component tests:

```bash
cargo test migration --lib
```

### Integration Tests

Run full workflow tests:

```bash
cargo test migration_integration_tests --test migration_integration_tests
```

### Performance Benchmarks

Run migration benchmarks:

```bash
cargo bench migration_benchmarks
```

## Best Practices

### Security

1. **Key Management**: Use proper key rotation and secure storage
2. **Network Security**: Use TLS for all migration communications
3. **Validation**: Always validate migration tokens and state integrity
4. **Monitoring**: Monitor for suspicious migration patterns

### Performance

1. **State Size**: Keep migrated state as small as possible
2. **Compression**: Use appropriate compression for your data
3. **Chunking**: Configure chunk sizes based on your network
4. **Concurrency**: Limit concurrent migrations to prevent resource exhaustion

### Reliability

1. **Health Checks**: Implement comprehensive health monitoring
2. **Timeouts**: Set appropriate timeouts for migration operations
3. **Rollback**: Always have a rollback strategy for failed migrations
4. **Testing**: Thoroughly test migration scenarios

## Troubleshooting

### Common Issues

**Migration Fails with "Invalid Token"**:
- Check token expiry configuration
- Verify secret key consistency across workers
- Check system clock synchronization

**State Corruption Detected**:
- Verify encryption key consistency
- Check network reliability during transfer
- Validate serialization/deserialization logic

**Target Worker Unavailable**:
- Check worker health and connectivity
- Verify load balancer configuration
- Check resource availability on target

**Performance Issues**:
- Monitor state size and compression ratios
- Check network bandwidth and latency
- Verify chunk size configuration

### Debug Mode

Enable detailed logging:

```bash
RUST_LOG=debug cargo run your_application
```

### Health Monitoring

Use the health checker for debugging:

```rust
let health_status = health_checker.check_migration_health(&request).await?;
println!("Health details: {:#?}", health_status);
```

## Future Enhancements

Planned improvements include:

- **Cross-Region Migration**: Support for geographic migration
- **Partial State Migration**: Migrate only specific state components
- **Migration Scheduling**: Advanced scheduling and orchestration
- **Custom Compression**: Pluggable compression algorithms
- **Migration Analytics**: Enhanced metrics and analysis

## Contributing

To contribute to the migration system:

1. Read the architecture documentation
2. Run the test suite: `cargo test migration`
3. Run benchmarks: `cargo bench migration`
4. Follow the security guidelines
5. Update documentation for any API changes

For questions or issues, please see the main project README or file an issue in the repository.