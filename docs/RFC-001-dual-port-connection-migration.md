# RFC-001: Dual-Port Architecture with Zero-Downtime QUIC Connection Migration

**RFC Number:** 001  
**Title:** Dual-Port Architecture with Zero-Downtime QUIC Connection Migration  
**Author:** rpcnet Team  
**Status:** Draft  
**Created:** 2025-09-26  
**Category:** Standards Track  

## Abstract

This RFC specifies a dual-port architecture for the rpcnet framework that enables zero-downtime migration of established QUIC connections between server processes. The design introduces a management port for control plane operations and a userspace port for business logic, enabling live connection handoff without requiring re-authentication or TLS renegotiation. This capability is critical for implementing hot code reloading, rolling updates, load balancing, and fault tolerance in distributed RPC systems.

## 1. Introduction

### 1.1 Motivation

Modern distributed systems require the ability to migrate active connections between processes for several critical use cases:

1. **Hot Code Reloading**: Update server logic without dropping client connections
2. **Rolling Updates**: Deploy new versions with zero downtime
3. **Dynamic Load Balancing**: Redistribute connections based on real-time metrics
4. **Fault Tolerance**: Migrate connections away from unhealthy instances
5. **Resource Optimization**: Consolidate connections during low-traffic periods

The current single-port architecture in rpcnet limits these capabilities, as connection migration requires clients to reconnect, causing service disruption and requiring expensive TLS handshakes.

### 1.2 Requirements

The solution must satisfy the following requirements:

- **R1**: Zero-downtime connection migration between processes
- **R2**: Preserve TLS session state during migration
- **R3**: Maintain stream continuity across migration
- **R4**: Support both same-host and cross-host migration
- **R5**: Provide atomic migration with rollback capability
- **R6**: Minimize migration latency (target: <10ms same-host, <50ms cross-host)
- **R7**: Ensure backward compatibility with existing rpcnet clients

### 1.3 Design Goals

1. **Separation of Concerns**: Clear distinction between control and data planes
2. **Security**: No weakening of TLS security guarantees
3. **Performance**: Minimal overhead for non-migrating connections
4. **Reliability**: Graceful handling of migration failures
5. **Observability**: Rich telemetry for migration events

## 2. Architecture Overview

### 2.1 Dual-Port Model

Each rpcnet server instance operates with two distinct UDP ports:

```
┌─────────────────────────────────────┐
│         RPC Server Instance         │
├─────────────────────────────────────┤
│  Management Port (UDP)              │
│  - Connection migration protocol    │
│  - Health checks                    │
│  - Metrics collection               │
│  - Service discovery                │
├─────────────────────────────────────┤
│  Userspace Port (UDP)               │
│  - Business logic RPCs              │
│  - Application streams              │
│  - Client connections               │
└─────────────────────────────────────┘
```

### 2.2 Connection State Model

A QUIC connection's state is composed of:

```rust
pub struct ConnectionState {
    // QUIC Transport State
    pub connection_id: ConnectionId,
    pub peer_address: SocketAddr,
    pub tls_session: TlsSession,
    pub congestion_state: CongestionControl,
    pub flow_control: FlowControl,
    
    // Stream State
    pub active_streams: HashMap<StreamId, StreamState>,
    pub stream_id_generator: StreamIdGenerator,
    
    // Security Context
    pub certificates: PeerCertificates,
    pub session_tickets: Vec<SessionTicket>,
    pub alpn: String,
    
    // Migration Context
    pub migration_token: Option<MigrationToken>,
    pub migration_version: u64,
}
```

### 2.3 Component Interactions

```
┌────────────────┐          Dual Channels           ┌─────────────────┐
│                │ ◄────── Userspace Port ────────► │                 │
│     Client     │         (Business Logic)         │    Server A     │
│                │ ◄────── Management Port ───────► │                 │
└────────┬───────┘         (Control Plane)          └────────┬────────┘
         │                                                    │
         │              Migration Scenarios:                 │
         │                                                    │
         ├─────── Client-Initiated Migration ────────────────┤
         │       1. Client requests migration                │
         │       2. Server A confirms (or auto-confirms)     │
         │       3. Migration protocol executes              │
         │                                                    │
         └─────── Server-Initiated Migration ────────────────┘
                 1. Server A initiates handoff
                 2. Client confirms (or auto-confirms)
                 3. Migration protocol executes

                        Migration Protocol Flow:
┌────────────────┐                                  ┌─────────────────┐
│     Client     │                                  │    Server A     │
│ ┌────────────┐ │         1. Migration Request    │ ┌─────────────┐ │
│ │ Userspace  │ │ ◄──────────────────────────────► │ │ Management │ │
│ │   Port     │ │                                  │ │    Port    │ │
│ └────────────┘ │         2. Confirmation         │ └─────────────┘ │
│ ┌────────────┐ │ ◄──────────────────────────────► │ ┌─────────────┐ │
│ │ Management │ │                                  │ │ Userspace  │ │
│ │   Port     │ │         3. State Freeze         │ │    Port    │ │
│ └────────────┘ │ ─────────────────────────────────│ └─────────────┘ │
└────────────────┘                                  └─────────┬───────┘
         │                                                    │
         │              4. State Transfer                    │
         │                    via                            ▼
         │              Management Ports            ┌─────────────────┐
         │                                          │    Server B     │
         │                                          │ ┌─────────────┐ │
         │         5. Connection Reconstruction    │ │ Management │ │
         └─────────────────────────────────────────►│ │    Port    │ │
                                                    │ └─────────────┘ │
                   6. Resume on Server B            │ ┌─────────────┐ │
         ◄───────────────────────────────────────────│ │ Userspace  │ │
                   (Both Ports Active)              │ │    Port    │ │
                                                    │ └─────────────┘ │
                                                    └─────────────────┘
```

## 3. Migration Protocol Specification

### 3.1 Protocol Messages

The migration protocol uses binary-encoded messages over UDP:

```rust
#[derive(Serialize, Deserialize)]
pub enum MigrationMessage {
    // Initiation Phase - Can originate from either client or server
    MigrationRequest {
        connection_id: ConnectionId,
        initiator: MigrationInitiator,
        target_server: SocketAddr,
        migration_token: [u8; 32],
        reason: MigrationReason,
        requested_at: SystemTime,
    },
    
    // Confirmation Phase - Required unless auto-confirm is enabled
    MigrationConfirmation {
        connection_id: ConnectionId,
        migration_token: [u8; 32],
        decision: ConfirmationDecision,
        confirmed_by: EndpointRole,
        metadata: Option<HashMap<String, String>>,
    },
    
    // Preparation Phase
    PrepareAccept {
        connection_id: ConnectionId,
        state_snapshot: ConnectionState,
        migration_token: [u8; 32],
    },
    
    PrepareAck {
        connection_id: ConnectionId,
        ready: bool,
        management_endpoint: SocketAddr,
    },
    
    // Execution Phase
    BeginMigration {
        connection_id: ConnectionId,
        freeze_streams: bool,
    },
    
    StateTransfer {
        connection_id: ConnectionId,
        encrypted_state: Vec<u8>,  // AES-256-GCM encrypted
        hmac: [u8; 32],            // HMAC-SHA256
    },
    
    // Completion Phase
    MigrationComplete {
        connection_id: ConnectionId,
        new_endpoint: SocketAddr,
        status: MigrationStatus,
    },
    
    // Rollback
    MigrationAbort {
        connection_id: ConnectionId,
        reason: String,
    },
}

#[derive(Serialize, Deserialize)]
pub enum MigrationStatus {
    Success,
    PartialSuccess { migrated_streams: Vec<StreamId> },
    Failed { error: String },
}

#[derive(Serialize, Deserialize)]
pub enum MigrationInitiator {
    Client { reason: String },
    Server { server_id: String, reason: String },
}

#[derive(Serialize, Deserialize)]
pub enum MigrationReason {
    ClientRequested,
    LoadBalancing,
    ServerMaintenance,
    ResourceOptimization,
    HealthCheckFailure,
    GeographicOptimization,
    Custom(String),
}

#[derive(Serialize, Deserialize)]
pub enum ConfirmationDecision {
    Accept,
    Reject { reason: String },
    Defer { retry_after: Duration },
}

#[derive(Serialize, Deserialize)]
pub enum EndpointRole {
    Client,
    Server,
}
```

### 3.2 Migration Phases

#### Phase 1: Initiation (T+0ms)

1. Migration trigger occurs (can be from either client or server):
   - **Client-initiated**: Client sends `MigrationRequest` via management port
   - **Server-initiated**: Server sends `MigrationRequest` via management port
2. Generate cryptographically secure migration token
3. Include initiator information and migration reason
4. Start migration timer (default timeout: 5 seconds)

#### Phase 2: Confirmation (T+2ms)

1. Non-initiating party receives `MigrationRequest`
2. Evaluates request based on:
   - Current connection state
   - Resource availability
   - Policy constraints
   - Security considerations
3. Sends `MigrationConfirmation` with decision:
   - **Accept**: Proceed with migration
   - **Reject**: Abort with reason
   - **Defer**: Retry after specified duration
4. If auto-confirm is enabled, skip to Phase 3

#### Phase 3: Preparation (T+5ms)

1. Target server validates migration request
2. Allocates resources for incoming connection
3. Sends `PrepareAck` indicating readiness
4. Source server freezes new stream creation

#### Phase 4: State Capture (T+10ms)

1. Source server captures connection state snapshot
2. Quiesces active streams (complete in-flight operations)
3. Serializes state including:
   - TLS session and tickets
   - QUIC transport parameters
   - Stream states and buffers
   - Congestion control state

#### Phase 5: State Transfer (T+15ms)

1. Encrypt state with ephemeral key (AES-256-GCM)
2. Calculate HMAC for integrity verification
3. Send `StateTransfer` message to target via management ports
4. Target server reconstructs connection state on both ports

#### Phase 6: Connection Pivot (T+20ms)

1. Target server injects connection into QUIC stack
2. Activates both userspace and management ports
3. Updates routing tables for both ports
4. Sends `MigrationComplete` to source
5. Source server marks connection as migrated

#### Phase 7: Client Notification (T+25ms)

1. Target server sends QUIC PATH_CHALLENGE frames on both ports
2. Client responds with PATH_RESPONSE frames on both ports
3. Dual-channel connection continues on new endpoints
4. Old endpoints closed after grace period

### 3.3 State Serialization Format

Connection state is serialized using a versioned binary format:

```
┌──────────────┬───────────────┬──────────────┬─────────────┐
│Version (4B)  │ Flags (4B)    │ Length (4B)  │ Checksum(4B)│
├──────────────┴───────────────┴──────────────┴─────────────┤
│                     TLS Session State                      │
├─────────────────────────────────────────────────────────────┤
│                    QUIC Transport State                    │
├─────────────────────────────────────────────────────────────┤
│                      Stream States                         │
├─────────────────────────────────────────────────────────────┤
│                    Application Context                     │
└─────────────────────────────────────────────────────────────┘
```

## 4. Implementation Details

### 4.1 Server Configuration

```rust
pub struct DualPortConfig {
    // Port Configuration
    pub userspace_port: u16,
    pub management_port: u16,
    
    // Migration Settings
    pub migration_timeout: Duration,
    pub state_encryption_key: [u8; 32],
    pub max_concurrent_migrations: usize,
    pub migration_buffer_size: usize,
    
    // Security
    pub require_migration_auth: bool,
    pub migration_hmac_key: [u8; 32],
    
    // Performance
    pub migration_thread_pool_size: usize,
    pub state_compression: CompressionAlgorithm,
}

impl Default for DualPortConfig {
    fn default() -> Self {
        Self {
            userspace_port: 8080,
            management_port: 8081,
            migration_timeout: Duration::from_secs(5),
            state_encryption_key: generate_key(),
            max_concurrent_migrations: 100,
            migration_buffer_size: 16 * 1024 * 1024, // 16MB
            require_migration_auth: true,
            migration_hmac_key: generate_key(),
            migration_thread_pool_size: 4,
            state_compression: CompressionAlgorithm::Zstd,
        }
    }
}
```

### 4.2 Migration Manager

```rust
pub struct MigrationManager {
    config: DualPortConfig,
    active_migrations: Arc<RwLock<HashMap<ConnectionId, MigrationState>>>,
    management_socket: Arc<UdpSocket>,
    metrics: Arc<MigrationMetrics>,
}

impl MigrationManager {
    pub async fn migrate_connection(
        &self,
        connection_id: ConnectionId,
        target: SocketAddr,
    ) -> Result<MigrationOutcome, MigrationError> {
        // Record migration start
        let migration_id = Uuid::new_v4();
        let start_time = Instant::now();
        
        // Phase 1: Initiate migration
        let token = self.generate_migration_token();
        self.send_migration_request(connection_id, target, token).await?;
        
        // Phase 2: Wait for target preparation
        let prep_ack = self.await_preparation_ack(connection_id).await?;
        if !prep_ack.ready {
            return Err(MigrationError::TargetNotReady);
        }
        
        // Phase 3: Capture connection state
        let state = self.capture_connection_state(connection_id).await?;
        
        // Phase 4: Transfer state
        let encrypted_state = self.encrypt_state(&state)?;
        self.transfer_state(target, connection_id, encrypted_state).await?;
        
        // Phase 5: Complete migration
        let outcome = self.complete_migration(connection_id, target).await?;
        
        // Record metrics
        self.metrics.record_migration(
            migration_id,
            start_time.elapsed(),
            outcome.clone(),
        );
        
        Ok(outcome)
    }
    
    async fn capture_connection_state(
        &self,
        connection_id: ConnectionId,
    ) -> Result<ConnectionState, MigrationError> {
        // Locate connection in QUIC stack
        let connection = self.find_connection(connection_id)?;
        
        // Freeze connection activity
        connection.freeze_streams().await?;
        
        // Extract state components
        let state = ConnectionState {
            connection_id,
            peer_address: connection.peer_address(),
            tls_session: connection.tls_session().clone(),
            congestion_state: connection.congestion_controller().snapshot(),
            flow_control: connection.flow_controller().snapshot(),
            active_streams: connection.streams().snapshot().await?,
            stream_id_generator: connection.stream_id_gen().clone(),
            certificates: connection.peer_certificates().clone(),
            session_tickets: connection.session_tickets().clone(),
            alpn: connection.alpn().to_string(),
            migration_token: Some(self.generate_migration_token()),
            migration_version: 1,
        };
        
        Ok(state)
    }
}
```

### 4.3 Connection Reconstruction

```rust
pub struct ConnectionReconstructor {
    quic_server: Arc<QuicServer>,
    tls_config: Arc<TlsConfig>,
}

impl ConnectionReconstructor {
    pub async fn reconstruct_connection(
        &self,
        state: ConnectionState,
    ) -> Result<QuicConnection, ReconstructionError> {
        // Create phantom connection
        let mut connection = self.quic_server.create_phantom_connection(
            state.connection_id,
            state.peer_address,
        )?;
        
        // Restore TLS session
        connection.restore_tls_session(
            state.tls_session,
            state.certificates,
            state.session_tickets,
        )?;
        
        // Restore transport state
        connection.restore_congestion_control(state.congestion_state)?;
        connection.restore_flow_control(state.flow_control)?;
        
        // Restore streams
        for (stream_id, stream_state) in state.active_streams {
            connection.restore_stream(stream_id, stream_state).await?;
        }
        
        // Restore ID generator
        connection.set_stream_id_generator(state.stream_id_generator);
        
        // Mark as migrated
        connection.set_migration_token(state.migration_token);
        
        // Inject into QUIC stack
        self.quic_server.inject_connection(connection.clone()).await?;
        
        Ok(connection)
    }
}
```

## 5. Security Considerations

### 5.1 Authentication and Authorization

1. **Migration Token**: Cryptographically secure 256-bit tokens prevent unauthorized migrations
2. **HMAC Verification**: All state transfers include HMAC-SHA256 for integrity
3. **TLS Session Continuity**: Migration preserves TLS session state, maintaining security context
4. **Certificate Pinning**: Target server must have same certificate as source

### 5.2 State Encryption

Connection state is encrypted using AES-256-GCM with:
- Ephemeral keys per migration
- Additional authenticated data (AAD) including connection ID and timestamps
- Key derivation using HKDF-SHA256

### 5.3 Threat Mitigation

| Threat | Mitigation |
|--------|------------|
| Connection Hijacking | Migration tokens + HMAC verification |
| State Tampering | AES-256-GCM encryption with authentication |
| Replay Attacks | Timestamp validation + nonce usage |
| Resource Exhaustion | Rate limiting + max concurrent migrations |
| Information Disclosure | TLS for all communications + state encryption |

## 6. Performance Analysis

### 6.1 Latency Breakdown

| Phase | Same-Host (ms) | Cross-Host (ms) | Cross-Region (ms) |
|-------|---------------|-----------------|-------------------|
| Initiation | 1 | 2 | 10 |
| Preparation | 2 | 5 | 15 |
| State Capture | 3 | 3 | 3 |
| State Transfer | 2 | 10 | 30 |
| Connection Pivot | 1 | 5 | 10 |
| Client Notification | 1 | 5 | 20 |
| **Total** | **10** | **30** | **88** |

### 6.2 Resource Overhead

- **Memory**: ~100KB per migration in flight
- **CPU**: <1% for migration manager thread
- **Network**: ~50KB state transfer per connection
- **Storage**: Optional state persistence adds ~1MB/s write throughput

### 6.3 Optimization Techniques

1. **State Compression**: Zstd compression reduces state size by ~60%
2. **Parallel Migrations**: Batch multiple connections in single transfer
3. **Incremental State**: Delta encoding for frequently migrating connections
4. **Connection Pooling**: Pre-established management connections

## 7. Testing Strategy

### 7.1 Unit Tests

```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_state_serialization_roundtrip() {
        let original_state = create_test_connection_state();
        let serialized = serialize_state(&original_state).unwrap();
        let deserialized = deserialize_state(&serialized).unwrap();
        assert_eq!(original_state, deserialized);
    }
    
    #[tokio::test]
    async fn test_migration_token_generation() {
        let manager = MigrationManager::new(Default::default());
        let token1 = manager.generate_migration_token();
        let token2 = manager.generate_migration_token();
        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 32);
    }
    
    #[tokio::test]
    async fn test_concurrent_migrations() {
        let manager = Arc::new(MigrationManager::new(Default::default()));
        let mut handles = vec![];
        
        for i in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.migrate_connection(
                    ConnectionId::from(i),
                    "127.0.0.1:9000".parse().unwrap(),
                ).await
            }));
        }
        
        for handle in handles {
            assert!(handle.await.is_ok());
        }
    }
}
```

### 7.2 Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_migration() {
    // Start two servers
    let server_a = start_test_server(8080, 8081).await;
    let server_b = start_test_server(9080, 9081).await;
    
    // Connect client to server A
    let client = RpcClient::connect("127.0.0.1:8080").await.unwrap();
    
    // Start streaming RPC
    let stream = client.call_streaming("test.stream", vec![]).await.unwrap();
    
    // Trigger migration
    server_a.migrate_connection(
        client.connection_id(),
        "127.0.0.1:9080".parse().unwrap(),
    ).await.unwrap();
    
    // Verify stream continues
    let result = stream.next().await;
    assert!(result.is_some());
    
    // Verify connection is on server B
    assert!(server_b.has_connection(client.connection_id()));
    assert!(!server_a.has_connection(client.connection_id()));
}
```

### 7.3 Chaos Testing

Implement fault injection to test:

1. **Network Failures**: Packet loss during migration
2. **Process Crashes**: Source/target server crashes mid-migration
3. **Resource Exhaustion**: Memory/CPU limits during migration
4. **Clock Skew**: Time synchronization issues
5. **Concurrent Operations**: Multiple migrations of same connection

### 7.4 Performance Benchmarks

```rust
#[bench]
fn bench_migration_latency(b: &mut Bencher) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let manager = MigrationManager::new(Default::default());
    
    b.iter(|| {
        runtime.block_on(async {
            manager.migrate_connection(
                ConnectionId::random(),
                "127.0.0.1:9000".parse().unwrap(),
            ).await
        })
    });
}
```

## 8. Monitoring and Observability

### 8.1 Metrics

```rust
pub struct MigrationMetrics {
    // Counters
    pub migrations_initiated: Counter,
    pub migrations_completed: Counter,
    pub migrations_failed: Counter,
    pub migrations_aborted: Counter,
    
    // Histograms
    pub migration_duration: Histogram,
    pub state_size: Histogram,
    pub encryption_time: Histogram,
    
    // Gauges
    pub active_migrations: Gauge,
    pub migration_queue_depth: Gauge,
}
```

### 8.2 Tracing

Integration with OpenTelemetry for distributed tracing:

```rust
#[instrument(skip(self))]
pub async fn migrate_connection(&self, ...) -> Result<...> {
    let span = tracing::info_span!(
        "migration",
        connection_id = %connection_id,
        target = %target,
    );
    
    // Migration logic with trace events
    tracing::event!(Level::INFO, "Starting migration");
    // ...
}
```

### 8.3 Health Checks

Management port exposes health endpoints:

```
GET /health/live    - Liveness probe
GET /health/ready   - Readiness probe  
GET /metrics        - Prometheus metrics
GET /migrations     - Active migration status
```

## 9. Backward Compatibility

### 9.1 Client Compatibility

The protocol maintains backward compatibility through:

1. **Version Negotiation**: Clients unaware of migration continue working
2. **Transparent Migration**: No client-side changes required
3. **Graceful Degradation**: Falls back to reconnection if migration fails

### 9.2 Server Compatibility

Migration between different server versions:

```rust
pub trait MigrationVersionAdapter {
    fn can_migrate_from(&self, version: u32) -> bool;
    fn adapt_state(&self, state: &mut ConnectionState, from_version: u32);
}
```

## 10. Future Extensions

### 10.1 Planned Enhancements

1. **Cross-Region Migration**: WAN-optimized state transfer
2. **Predictive Migration**: ML-based migration scheduling
3. **State Persistence**: Durable state for crash recovery
4. **Connection Pooling**: Pre-warmed connections for faster migration
5. **Multi-Path Migration**: Parallel state transfer over multiple paths

### 10.2 Ecosystem Integration

- Kubernetes operator for automated migration
- Service mesh integration (Istio/Linkerd)
- Cloud provider load balancer support
- Observability platform integrations

## 11. References

1. [IETF QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html)
2. [IETF QUIC Connection Migration](https://www.rfc-editor.org/rfc/rfc9000.html#name-connection-migration)
3. [s2n-quic Documentation](https://github.com/aws/s2n-quic)
4. [TLS 1.3 Session Resumption](https://www.rfc-editor.org/rfc/rfc8446.html#section-2.2)
5. [HKDF RFC 5869](https://www.rfc-editor.org/rfc/rfc5869.html)

## 12. Acknowledgments

This RFC builds upon the existing rpcnet framework and incorporates lessons learned from production deployments of QUIC-based services. Special thanks to the s2n-quic team for their robust QUIC implementation.

## Appendix A: State Machine

```
┌─────────┐
│  IDLE   │
└────┬────┘
     │ MigrationRequest
     ▼
┌─────────────┐
│ PREPARING   │
└────┬────────┘
     │ PrepareAck
     ▼
┌─────────────┐
│  CAPTURING  │
└────┬────────┘
     │ StateCapture
     ▼
┌──────────────┐
│ TRANSFERRING │
└────┬─────────┘
     │ StateTransfer
     ▼
┌─────────────┐
│  PIVOTING   │
└────┬────────┘
     │ Success/Failure
     ▼
┌─────────────┐     ┌─────────┐
│  COMPLETED  │     │ ABORTED │
└─────────────┘     └─────────┘
```

## Appendix B: Configuration Example

```toml
[rpcnet.dual_port]
userspace_port = 8080
management_port = 8081

[rpcnet.migration]
timeout_ms = 5000
max_concurrent = 100
buffer_size_mb = 16
compression = "zstd"
encryption = "aes256gcm"

[rpcnet.migration.security]
require_auth = true
hmac_algorithm = "sha256"
token_rotation_interval = "1h"

[rpcnet.migration.performance]
thread_pool_size = 4
batch_size = 10
state_cache_ttl = "5m"
```

---

*End of RFC-001*