use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotState {
    Capturing,
    Ready,
    Transferred,
    Applied,
    Expired,
}

impl Default for SnapshotState {
    fn default() -> Self {
        SnapshotState::Capturing
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportState {
    pub connection_id: Vec<u8>,
    pub sequence_numbers: HashMap<u64, u64>, // StreamId -> sequence number
    pub rtt_estimate: Duration,
    pub bandwidth_estimate: u64,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            connection_id: Vec::new(),
            sequence_numbers: HashMap::new(),
            rtt_estimate: Duration::from_millis(50),
            bandwidth_estimate: 1_000_000, // 1 Mbps default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CongestionState {
    pub congestion_window: u64,
    pub ssthresh: u64,
    pub rtt_variance: Duration,
    pub min_rtt: Duration,
    pub packets_in_flight: u32,
}

impl Default for CongestionState {
    fn default() -> Self {
        Self {
            congestion_window: 10,
            ssthresh: u64::MAX,
            rtt_variance: Duration::from_millis(10),
            min_rtt: Duration::from_millis(20),
            packets_in_flight: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlState {
    pub connection_send_window: u64,
    pub connection_recv_window: u64,
    pub stream_windows: HashMap<u64, (u64, u64)>, // StreamId -> (send_window, recv_window)
}

impl Default for FlowControlState {
    fn default() -> Self {
        Self {
            connection_send_window: 1_048_576, // 1MB
            connection_recv_window: 1_048_576, // 1MB
            stream_windows: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub tls_session_ticket: Vec<u8>,
    pub encryption_keys: Vec<u8>,
    pub certificate_chain: Vec<u8>,
    pub cipher_suite: String,
    pub protocol_version: String,
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self {
            tls_session_ticket: Vec::new(),
            encryption_keys: Vec::new(),
            certificate_chain: Vec::new(),
            cipher_suite: "TLS_AES_256_GCM_SHA384".to_string(),
            protocol_version: "TLSv1.3".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStateSnapshot {
    pub snapshot_id: Uuid,
    pub session_id: Uuid,
    pub captured_at: SystemTime,
    pub state: SnapshotState,
    pub transport_state: TransportState,
    pub congestion_state: CongestionState,
    pub flow_control_state: FlowControlState,
    pub security_context: SecurityContext,
    pub stream_buffers: HashMap<u64, Vec<u8>>, // StreamId -> buffer data
    pub serialized_size: usize,
    pub checksum: [u8; 32], // HMAC-SHA256
}

impl ConnectionStateSnapshot {
    pub fn new(session_id: Uuid) -> Self {
        Self {
            snapshot_id: Uuid::new_v4(),
            session_id,
            captured_at: SystemTime::now(),
            state: SnapshotState::default(),
            transport_state: TransportState::default(),
            congestion_state: CongestionState::default(),
            flow_control_state: FlowControlState::default(),
            security_context: SecurityContext::default(),
            stream_buffers: HashMap::new(),
            serialized_size: 0,
            checksum: [0u8; 32],
        }
    }

    pub fn with_transport_state(mut self, transport_state: TransportState) -> Self {
        self.transport_state = transport_state;
        self
    }

    pub fn with_congestion_state(mut self, congestion_state: CongestionState) -> Self {
        self.congestion_state = congestion_state;
        self
    }

    pub fn with_flow_control_state(mut self, flow_control_state: FlowControlState) -> Self {
        self.flow_control_state = flow_control_state;
        self
    }

    pub fn with_security_context(mut self, security_context: SecurityContext) -> Self {
        self.security_context = security_context;
        self
    }

    pub fn add_stream_buffer(mut self, stream_id: u64, buffer: Vec<u8>) -> Self {
        self.stream_buffers.insert(stream_id, buffer);
        self
    }

    pub fn mark_ready(&mut self) {
        self.state = SnapshotState::Ready;
        self.update_serialized_size();
    }

    pub fn mark_transferred(&mut self) {
        if self.state == SnapshotState::Ready {
            self.state = SnapshotState::Transferred;
        }
    }

    pub fn mark_applied(&mut self) {
        if self.state == SnapshotState::Transferred {
            self.state = SnapshotState::Applied;
        }
    }

    pub fn mark_expired(&mut self) {
        self.state = SnapshotState::Expired;
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, SnapshotState::Ready)
    }

    pub fn is_transferred(&self) -> bool {
        matches!(self.state, SnapshotState::Transferred)
    }

    pub fn is_applied(&self) -> bool {
        matches!(self.state, SnapshotState::Applied)
    }

    pub fn is_expired(&self) -> bool {
        matches!(self.state, SnapshotState::Expired)
    }

    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.captured_at)
            .unwrap_or(Duration::ZERO)
    }

    pub fn is_expired_by_time(&self, max_age: Duration) -> bool {
        self.age() > max_age
    }

    pub fn total_stream_data_size(&self) -> usize {
        self.stream_buffers.values().map(|buf| buf.len()).sum()
    }

    pub fn stream_count(&self) -> usize {
        self.stream_buffers.len()
    }

    fn update_serialized_size(&mut self) {
        // Estimate serialized size based on data structures
        let base_size = std::mem::size_of::<Self>();
        let transport_overhead = self.transport_state.connection_id.len() 
            + self.transport_state.sequence_numbers.len() * (8 + 8); // StreamId + sequence
        let security_overhead = self.security_context.tls_session_ticket.len()
            + self.security_context.encryption_keys.len()
            + self.security_context.certificate_chain.len()
            + self.security_context.cipher_suite.len()
            + self.security_context.protocol_version.len();
        let stream_data_size = self.total_stream_data_size();
        let flow_control_overhead = self.flow_control_state.stream_windows.len() * (8 + 8 + 8); // StreamId + send + recv windows

        self.serialized_size = base_size 
            + transport_overhead 
            + security_overhead 
            + stream_data_size 
            + flow_control_overhead;
    }

    pub fn update_checksum(&mut self, hmac_key: &[u8]) -> Result<(), String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // Serialize the snapshot without the checksum field
        let mut temp_snapshot = self.clone();
        temp_snapshot.checksum = [0u8; 32];
        
        let serialized = bincode::serialize(&temp_snapshot)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;

        let mut mac = HmacSha256::new_from_slice(hmac_key)
            .map_err(|e| format!("Invalid HMAC key: {}", e))?;
        mac.update(&serialized);
        
        let result = mac.finalize().into_bytes();
        self.checksum.copy_from_slice(&result[..32]);
        
        Ok(())
    }

    pub fn verify_checksum(&self, hmac_key: &[u8]) -> Result<bool, String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // Serialize the snapshot without the checksum field
        let mut temp_snapshot = self.clone();
        temp_snapshot.checksum = [0u8; 32];
        
        let serialized = bincode::serialize(&temp_snapshot)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;

        let mut mac = HmacSha256::new_from_slice(hmac_key)
            .map_err(|e| format!("Invalid HMAC key: {}", e))?;
        mac.update(&serialized);
        
        let expected = mac.finalize().into_bytes();
        Ok(&expected[..32] == &self.checksum)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.stream_buffers.len() > 1000 {
            return Err("Too many streams in snapshot".to_string());
        }

        if self.serialized_size > 10_485_760 { // 10MB limit
            return Err("Snapshot size exceeds limit".to_string());
        }

        if self.total_stream_data_size() > 5_242_880 { // 5MB limit for stream data
            return Err("Stream data size exceeds limit".to_string());
        }

        if self.age() > Duration::from_secs(300) { // 5 minute max age
            return Err("Snapshot too old".to_string());
        }

        // Validate security context
        if self.security_context.tls_session_ticket.is_empty() {
            return Err("TLS session ticket is required".to_string());
        }

        // Validate transport state
        if self.transport_state.connection_id.is_empty() {
            return Err("Connection ID is required".to_string());
        }

        Ok(())
    }

    pub fn estimate_transfer_time(&self, bandwidth_bps: u64) -> Duration {
        if bandwidth_bps == 0 {
            return Duration::from_secs(3600); // 1 hour fallback
        }

        let transfer_time_secs = (self.serialized_size as u64 * 8) / bandwidth_bps;
        Duration::from_secs(transfer_time_secs.max(1))
    }

    pub fn compress(&mut self) -> Result<(), String> {
        // Compress large stream buffers to reduce transfer size
        for (stream_id, buffer) in &mut self.stream_buffers {
            if buffer.len() > 1024 { // Only compress buffers > 1KB
                // Simple compression placeholder - in practice would use a real compression library
                // For now, just truncate extremely large buffers as a safety measure
                if buffer.len() > 65536 { // 64KB limit per stream
                    buffer.truncate(65536);
                }
            }
        }
        
        self.update_serialized_size();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_snapshot() {
        let session_id = Uuid::new_v4();
        let snapshot = ConnectionStateSnapshot::new(session_id);

        assert_eq!(snapshot.session_id, session_id);
        assert_eq!(snapshot.state, SnapshotState::Capturing);
        assert_eq!(snapshot.stream_buffers.len(), 0);
        assert_eq!(snapshot.serialized_size, 0);
    }

    #[test]
    fn test_builder_pattern() {
        let session_id = Uuid::new_v4();
        let transport_state = TransportState {
            connection_id: vec![1, 2, 3, 4],
            ..Default::default()
        };

        let snapshot = ConnectionStateSnapshot::new(session_id)
            .with_transport_state(transport_state.clone())
            .add_stream_buffer(1, vec![1, 2, 3, 4, 5]);

        assert_eq!(snapshot.transport_state.connection_id, vec![1, 2, 3, 4]);
        assert_eq!(snapshot.stream_buffers.get(&1), Some(&vec![1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_state_transitions() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id);

        assert_eq!(snapshot.state, SnapshotState::Capturing);
        assert!(!snapshot.is_ready());

        snapshot.mark_ready();
        assert_eq!(snapshot.state, SnapshotState::Ready);
        assert!(snapshot.is_ready());

        snapshot.mark_transferred();
        assert_eq!(snapshot.state, SnapshotState::Transferred);
        assert!(snapshot.is_transferred());

        snapshot.mark_applied();
        assert_eq!(snapshot.state, SnapshotState::Applied);
        assert!(snapshot.is_applied());
    }

    #[test]
    fn test_size_calculations() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id)
            .add_stream_buffer(1, vec![0; 1000])
            .add_stream_buffer(2, vec![0; 2000]);

        snapshot.mark_ready(); // This updates serialized_size

        assert_eq!(snapshot.total_stream_data_size(), 3000);
        assert_eq!(snapshot.stream_count(), 2);
        assert!(snapshot.serialized_size > 3000); // Should include overhead
    }

    #[test]
    fn test_validation() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id);

        // Should fail validation - missing required fields
        assert!(snapshot.validate().is_err());

        // Add required fields
        snapshot.transport_state.connection_id = vec![1, 2, 3, 4];
        snapshot.security_context.tls_session_ticket = vec![1, 2, 3, 4, 5];

        // Should pass validation now
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn test_checksum_operations() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id);
        let hmac_key = b"test_key_32_bytes_long_padding!!";

        // Update checksum
        assert!(snapshot.update_checksum(hmac_key).is_ok());
        assert_ne!(snapshot.checksum, [0u8; 32]);

        // Verify checksum
        assert!(snapshot.verify_checksum(hmac_key).unwrap());

        // Modify data and verify checksum fails
        snapshot.session_id = Uuid::new_v4();
        assert!(!snapshot.verify_checksum(hmac_key).unwrap());
    }

    #[test]
    fn test_expiration() {
        let session_id = Uuid::new_v4();
        let snapshot = ConnectionStateSnapshot::new(session_id);

        assert!(!snapshot.is_expired_by_time(Duration::from_secs(1)));
        assert!(snapshot.age() < Duration::from_secs(1));
    }

    #[test]
    fn test_compression() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id)
            .add_stream_buffer(1, vec![0; 100000]); // Large buffer

        let original_size = snapshot.total_stream_data_size();
        assert!(snapshot.compress().is_ok());
        
        // Should be truncated to 64KB limit
        assert_eq!(snapshot.stream_buffers.get(&1).unwrap().len(), 65536);
        assert!(snapshot.total_stream_data_size() < original_size);
    }

    #[test]
    fn test_transfer_time_estimation() {
        let session_id = Uuid::new_v4();
        let mut snapshot = ConnectionStateSnapshot::new(session_id);
        snapshot.serialized_size = 1_000_000; // 1MB

        let bandwidth_1mbps = 1_000_000; // 1 Mbps
        let transfer_time = snapshot.estimate_transfer_time(bandwidth_1mbps);
        
        // Should be approximately 8 seconds (8 bits per byte)
        assert!(transfer_time >= Duration::from_secs(7));
        assert!(transfer_time <= Duration::from_secs(9));
    }
}