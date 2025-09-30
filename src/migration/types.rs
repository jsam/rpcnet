use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

// Re-export models from the models module to avoid duplication
pub use crate::migration::models::{
    ConnectionSession, ConnectionState, MigrationStage, ConnectionMetrics,
    MigrationRequest, MigrationReason, Priority, ServerInstance,
    ConnectionStateSnapshot, SnapshotState, TransportState, CongestionState, FlowControlState, SecurityContext,
    MigrationConfirmation, ConfirmationDecision, MigrationParty, ConfirmationState,
    MigrationToken, TokenPurpose, TokenState, ServerEndpoint,
};

// Migration Protocol Enums

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPhase {
    Initiation,
    StateCapture,
    StateTransfer,
    ConnectionEstablishment,
    StateApplication,
    Verification,
    Completion,
    Rollback,
}

impl Default for MigrationPhase {
    fn default() -> Self {
        MigrationPhase::Initiation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationOutcome {
    Success,
    Failed(MigrationFailureReason),
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationFailureReason {
    NetworkError,
    TargetServerUnreachable,
    StateTransferFailed,
    ConnectionEstablishmentFailed,
    StateApplicationFailed,
    VerificationFailed,
    TokenValidationFailed,
    InsufficientResources,
    IncompatibleVersions,
    SecurityViolation,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    Userspace,
    Management,
    Migration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerRole {
    Source,
    Target,
    Director,
    Worker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    V1_0,
    V1_1,
    V2_0,
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        ProtocolVersion::V2_0
    }
}

impl ProtocolVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolVersion::V1_0 => "1.0",
            ProtocolVersion::V1_1 => "1.1", 
            ProtocolVersion::V2_0 => "2.0",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "1.0" => Ok(ProtocolVersion::V1_0),
            "1.1" => Ok(ProtocolVersion::V1_1),
            "2.0" => Ok(ProtocolVersion::V2_0),
            _ => Err(format!("Unknown protocol version: {}", s)),
        }
    }

    pub fn is_compatible(&self, other: &ProtocolVersion) -> bool {
        match (self, other) {
            (ProtocolVersion::V2_0, _) => true, // V2.0 is backward compatible
            (ProtocolVersion::V1_1, ProtocolVersion::V1_1) => true,
            (ProtocolVersion::V1_1, ProtocolVersion::V1_0) => true,
            (ProtocolVersion::V1_0, ProtocolVersion::V1_0) => true,
            _ => false,
        }
    }
}

// Migration Protocol Messages

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMessage {
    pub message_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: SystemTime,
    pub source_server: Uuid,
    pub target_server: Uuid,
    pub protocol_version: ProtocolVersion,
    pub message_type: MigrationMessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationMessageType {
    InitiationRequest(MigrationRequest),
    InitiationResponse(MigrationInitiationResponse),
    StateTransferRequest(StateTransferRequest),
    StateTransferResponse(StateTransferResponse),
    ConnectionEstablishmentRequest(ConnectionEstablishmentRequest),
    ConnectionEstablishmentResponse(ConnectionEstablishmentResponse),
    VerificationRequest(VerificationRequest),
    VerificationResponse(VerificationResponse),
    CompletionNotification(CompletionNotification),
    RollbackRequest(RollbackRequest),
    RollbackResponse(RollbackResponse),
    HealthCheck(HealthCheckMessage),
    StatusUpdate(StatusUpdateMessage),
    ErrorNotification(ErrorNotification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationInitiationResponse {
    pub request_id: Uuid,
    pub accepted: bool,
    pub reason: Option<String>,
    pub estimated_duration: Option<Duration>,
    pub supported_protocol_version: ProtocolVersion,
    pub migration_token: Option<MigrationToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransferRequest {
    pub migration_token: MigrationToken,
    pub snapshot: ConnectionStateSnapshot,
    pub transfer_method: StateTransferMethod,
    pub chunk_index: Option<u32>,
    pub total_chunks: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransferResponse {
    pub request_id: Uuid,
    pub success: bool,
    pub error: Option<String>,
    pub bytes_received: usize,
    pub checksum_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateTransferMethod {
    Direct,
    Chunked,
    Compressed,
    Encrypted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionEstablishmentRequest {
    pub migration_token: MigrationToken,
    pub connection_parameters: ConnectionParameters,
    pub tls_config: TlsConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionEstablishmentResponse {
    pub request_id: Uuid,
    pub success: bool,
    pub connection_id: Option<Uuid>,
    pub endpoint: Option<ServerEndpoint>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub migration_token: MigrationToken,
    pub verification_data: VerificationData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResponse {
    pub request_id: Uuid,
    pub verified: bool,
    pub error: Option<String>,
    pub performance_metrics: Option<PerformanceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionNotification {
    pub migration_token: MigrationToken,
    pub outcome: MigrationOutcome,
    pub duration: Duration,
    pub final_metrics: MigrationMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRequest {
    pub migration_token: MigrationToken,
    pub reason: MigrationFailureReason,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResponse {
    pub request_id: Uuid,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckMessage {
    pub server_id: Uuid,
    pub health_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdateMessage {
    pub migration_token: MigrationToken,
    pub phase: MigrationPhase,
    pub progress_percentage: f32,
    pub estimated_remaining: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorNotification {
    pub error_id: Uuid,
    pub migration_token: Option<MigrationToken>,
    pub error_code: MigrationFailureReason,
    pub message: String,
    pub recoverable: bool,
}

// Supporting Data Structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionParameters {
    pub quic_config: QuicConfiguration,
    pub flow_control_limits: FlowControlLimits,
    pub congestion_control: CongestionControlType,
    pub idle_timeout: Duration,
    pub keep_alive_interval: Duration,
}

impl Default for ConnectionParameters {
    fn default() -> Self {
        Self {
            quic_config: QuicConfiguration::default(),
            flow_control_limits: FlowControlLimits::default(),
            congestion_control: CongestionControlType::BBR,
            idle_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicConfiguration {
    pub max_concurrent_streams: u32,
    pub max_data: u64,
    pub max_stream_data: u64,
    pub initial_max_data: u64,
    pub initial_max_stream_data_bidi_local: u64,
    pub initial_max_stream_data_bidi_remote: u64,
    pub initial_max_stream_data_uni: u64,
}

impl Default for QuicConfiguration {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 100,
            max_data: 10_485_760, // 10MB
            max_stream_data: 1_048_576, // 1MB
            initial_max_data: 1_048_576, // 1MB
            initial_max_stream_data_bidi_local: 262_144, // 256KB
            initial_max_stream_data_bidi_remote: 262_144, // 256KB
            initial_max_stream_data_uni: 262_144, // 256KB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlLimits {
    pub connection_window: u64,
    pub stream_window: u64,
    pub max_buffer_size: usize,
}

impl Default for FlowControlLimits {
    fn default() -> Self {
        Self {
            connection_window: 1_048_576, // 1MB
            stream_window: 262_144, // 256KB
            max_buffer_size: 2_097_152, // 2MB
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CongestionControlType {
    Reno,
    Cubic,
    BBR,
    BBR2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfiguration {
    pub cipher_suites: Vec<String>,
    pub supported_versions: Vec<String>,
    pub certificate_chain: Vec<u8>,
    pub verify_peer: bool,
    pub alpn_protocols: Vec<String>,
}

impl Default for TlsConfiguration {
    fn default() -> Self {
        Self {
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                "TLS_AES_128_GCM_SHA256".to_string(),
            ],
            supported_versions: vec!["TLSv1.3".to_string()],
            certificate_chain: Vec::new(),
            verify_peer: true,
            alpn_protocols: vec!["rpcnet/2.0".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationData {
    pub connection_test_results: Vec<ConnectionTestResult>,
    pub performance_benchmarks: Vec<PerformanceBenchmark>,
    pub integrity_checks: Vec<IntegrityCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub test_id: Uuid,
    pub test_type: ConnectionTestType,
    pub success: bool,
    pub latency_ms: Option<f64>,
    pub throughput_mbps: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTestType {
    Ping,
    Echo,
    Throughput,
    Latency,
    PacketLoss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmark {
    pub benchmark_id: Uuid,
    pub benchmark_type: BenchmarkType,
    pub baseline_value: f64,
    pub measured_value: f64,
    pub acceptable_deviation: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkType {
    Latency,
    Throughput,
    CpuUsage,
    MemoryUsage,
    ConnectionCount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub check_id: Uuid,
    pub check_type: IntegrityCheckType,
    pub expected_hash: String,
    pub actual_hash: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityCheckType {
    StateChecksum,
    ConfigurationHash,
    CertificateFingerprint,
    StreamBufferHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub throughput_mbps: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub packet_loss_rate: f64,
    pub connection_establishment_time_ms: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            latency_p50_ms: 0.0,
            latency_p95_ms: 0.0,
            latency_p99_ms: 0.0,
            throughput_mbps: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0.0,
            packet_loss_rate: 0.0,
            connection_establishment_time_ms: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub server_id: Uuid,
    pub server_role: ServerRole,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub active_connections: u32,
    pub status: HealthState,
    pub uptime: Duration,
    pub last_health_check: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl Default for HealthState {
    fn default() -> Self {
        HealthState::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMetrics {
    pub migration_id: Uuid,
    pub server_id: Uuid,
    pub total_migrations: u64,
    pub successful_migrations: u64,
    pub failed_migrations: u64,
    pub cancelled_migrations: u64,
    pub average_migration_time: Duration,
    pub total_data_transferred_bytes: u64,
    pub peak_bandwidth_mbps: f64,
    pub average_bandwidth_mbps: f64,
}

impl Default for MigrationMetrics {
    fn default() -> Self {
        Self {
            migration_id: Uuid::new_v4(),
            server_id: Uuid::new_v4(),
            total_migrations: 0,
            successful_migrations: 0,
            failed_migrations: 0,
            cancelled_migrations: 0,
            average_migration_time: Duration::ZERO,
            total_data_transferred_bytes: 0,
            peak_bandwidth_mbps: 0.0,
            average_bandwidth_mbps: 0.0,
        }
    }
}

// Configuration and Setup Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server_id: Uuid,
    pub server_role: ServerRole,
    pub userspace_port: PortConfig,
    pub management_port: PortConfig,
    pub certificate_path: String,
    pub private_key_path: String,
    pub protocol_version: ProtocolVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub port: u16,
    pub bind_address: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            port: 0,
            bind_address: "127.0.0.1".to_string(),
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub client_id: Uuid,
    pub connection_type: ConnectionType,
    pub server_address: String,
    pub management_address: String,
    pub protocol_version: ProtocolVersion,
    pub connection_parameters: ConnectionParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResponse {
    pub request_id: Uuid,
    pub userspace_connection_id: Uuid,
    pub management_connection_id: Uuid,
    pub connection_established: bool,
    pub server_endpoint: Option<ServerEndpoint>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualConnection {
    pub userspace_connection: Uuid,
    pub management_connection: Uuid,
    pub established_at: SystemTime,
    pub last_activity: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_compatibility() {
        assert!(ProtocolVersion::V2_0.is_compatible(&ProtocolVersion::V1_0));
        assert!(ProtocolVersion::V2_0.is_compatible(&ProtocolVersion::V1_1));
        assert!(ProtocolVersion::V1_1.is_compatible(&ProtocolVersion::V1_0));
        assert!(!ProtocolVersion::V1_0.is_compatible(&ProtocolVersion::V1_1));
        assert!(!ProtocolVersion::V1_0.is_compatible(&ProtocolVersion::V2_0));
    }

    #[test]
    fn test_protocol_version_string_conversion() {
        assert_eq!(ProtocolVersion::V2_0.as_str(), "2.0");
        assert_eq!(ProtocolVersion::from_str("2.0").unwrap(), ProtocolVersion::V2_0);
        assert!(ProtocolVersion::from_str("3.0").is_err());
    }

    #[test]
    fn test_migration_message_creation() {
        let source_server = ServerInstance::new("127.0.0.1".to_string(), 8080, 8081);
        let target_server = ServerInstance::new("127.0.0.1".to_string(), 8082, 8083);
        
        let request = MigrationRequest::new(
            Uuid::new_v4(),
            source_server,
            target_server,
            MigrationReason::LoadBalancing,
            "test_system".to_string(),
        );
        
        let message = MigrationMessage {
            message_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            source_server: Uuid::new_v4(),
            target_server: Uuid::new_v4(),
            protocol_version: ProtocolVersion::default(),
            message_type: MigrationMessageType::InitiationRequest(request),
        };

        assert!(matches!(message.message_type, MigrationMessageType::InitiationRequest(_)));
    }

    #[test]
    fn test_health_status_defaults() {
        let health = HealthStatus {
            server_id: Uuid::new_v4(),
            server_role: ServerRole::Worker,
            cpu_usage_percent: 50.0,
            memory_usage_percent: 60.0,
            active_connections: 100,
            status: HealthState::default(),
            uptime: Duration::from_secs(3600),
            last_health_check: SystemTime::now(),
        };

        assert_eq!(health.status, HealthState::Unknown);
    }

    #[test]
    fn test_connection_parameters_defaults() {
        let params = ConnectionParameters::default();
        assert_eq!(params.congestion_control, CongestionControlType::BBR);
        assert_eq!(params.idle_timeout, Duration::from_secs(30));
        assert!(params.quic_config.max_concurrent_streams > 0);
    }

    #[test]
    fn test_migration_outcome_serialization() {
        let success = MigrationOutcome::Success;
        let failed = MigrationOutcome::Failed(MigrationFailureReason::NetworkError);
        
        let success_json = serde_json::to_string(&success).unwrap();
        let failed_json = serde_json::to_string(&failed).unwrap();
        
        let success_deser: MigrationOutcome = serde_json::from_str(&success_json).unwrap();
        let failed_deser: MigrationOutcome = serde_json::from_str(&failed_json).unwrap();
        
        assert_eq!(success, success_deser);
        assert_eq!(failed, failed_deser);
    }
}