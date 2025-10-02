use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Active,
    Migrating,
    Completed,
    Failed,
    RolledBack,
    Migrated,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStage {
    Initiated,
    Confirmed,
    TransferringState,
    Finalizing,
    Completed,
    Failed,
    Preparing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    pub bytes_transferred: u64,
    pub requests_served: u64,
    pub last_activity: SystemTime,
    pub average_latency: Duration,
    pub error_count: u32,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            bytes_transferred: 0,
            requests_served: 0,
            last_activity: SystemTime::now(),
            average_latency: Duration::from_millis(0),
            error_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSession {
    pub id: Uuid,
    pub client_id: Uuid,
    pub connection_state: ConnectionState,
    pub migration_stage: Option<MigrationStage>,
    pub source_server_id: Uuid,
    pub target_server_id: Option<Uuid>,
    pub created_at: SystemTime,
    pub last_updated: SystemTime,
    pub migration_token: Option<String>,
    pub state_snapshot_id: Option<Uuid>,
    pub metrics: ConnectionMetrics,
    pub metadata: HashMap<String, String>,
}

impl ConnectionSession {
    pub fn new(connection_id: Uuid) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::new_v4(),
            client_id: connection_id,
            connection_state: ConnectionState::Active,
            migration_stage: None,
            source_server_id: Uuid::new_v4(),
            target_server_id: None,
            created_at: now,
            last_updated: now,
            migration_token: None,
            state_snapshot_id: None,
            metrics: ConnectionMetrics::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_servers(connection_id: Uuid, source_server_id: Uuid) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::new_v4(),
            client_id: connection_id,
            connection_state: ConnectionState::Active,
            migration_stage: None,
            source_server_id,
            target_server_id: None,
            created_at: now,
            last_updated: now,
            migration_token: None,
            state_snapshot_id: None,
            metrics: ConnectionMetrics::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn initiate_migration(&mut self, target_server_id: Uuid, migration_token: String) {
        self.connection_state = ConnectionState::Migrating;
        self.migration_stage = Some(MigrationStage::Initiated);
        self.target_server_id = Some(target_server_id);
        self.migration_token = Some(migration_token);
        self.last_updated = SystemTime::now();
    }

    pub fn confirm_migration(&mut self) {
        if self.migration_stage == Some(MigrationStage::Initiated) {
            self.migration_stage = Some(MigrationStage::Confirmed);
            self.last_updated = SystemTime::now();
        }
    }

    pub fn start_state_transfer(&mut self, state_snapshot_id: Uuid) {
        if self.migration_stage == Some(MigrationStage::Confirmed) {
            self.migration_stage = Some(MigrationStage::TransferringState);
            self.state_snapshot_id = Some(state_snapshot_id);
            self.last_updated = SystemTime::now();
        }
    }

    pub fn finalize_migration(&mut self) {
        if self.migration_stage == Some(MigrationStage::TransferringState) {
            self.migration_stage = Some(MigrationStage::Finalizing);
            self.last_updated = SystemTime::now();
        }
    }

    pub fn complete_migration(&mut self) {
        if self.migration_stage == Some(MigrationStage::Finalizing) {
            self.connection_state = ConnectionState::Completed;
            self.migration_stage = Some(MigrationStage::Completed);
            if let Some(target_id) = self.target_server_id {
                self.source_server_id = target_id;
                self.target_server_id = None;
            }
            self.migration_token = None;
            self.state_snapshot_id = None;
            self.last_updated = SystemTime::now();
        }
    }

    pub fn fail_migration(&mut self, error_msg: &str) {
        self.connection_state = ConnectionState::Failed;
        self.migration_stage = Some(MigrationStage::Failed);
        self.target_server_id = None;
        self.metadata.insert("error".to_string(), error_msg.to_string());
        self.last_updated = SystemTime::now();
    }

    pub fn rollback_migration(&mut self) {
        self.connection_state = ConnectionState::RolledBack;
        self.migration_stage = None;
        self.target_server_id = None;
        self.migration_token = None;
        self.state_snapshot_id = None;
        self.last_updated = SystemTime::now();
    }

    pub fn update_metrics(&mut self, bytes_transferred: u64, latency: Duration) {
        self.metrics.bytes_transferred += bytes_transferred;
        self.metrics.requests_served += 1;
        self.metrics.last_activity = SystemTime::now();
        
        // Calculate moving average for latency
        let current_avg = self.metrics.average_latency.as_millis() as f64;
        let new_latency = latency.as_millis() as f64;
        let count = self.metrics.requests_served as f64;
        let new_avg = (current_avg * (count - 1.0) + new_latency) / count;
        self.metrics.average_latency = Duration::from_millis(new_avg as u64);
        
        self.last_updated = SystemTime::now();
    }

    pub fn increment_error_count(&mut self) {
        self.metrics.error_count += 1;
        self.last_updated = SystemTime::now();
    }

    pub fn is_migrating(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Migrating)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Completed)
    }

    pub fn can_migrate(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Active | ConnectionState::RolledBack)
    }

    pub fn age(&self) -> Duration {
        SystemTime::now().duration_since(self.created_at).unwrap_or(Duration::ZERO)
    }

    pub fn time_since_last_update(&self) -> Duration {
        SystemTime::now().duration_since(self.last_updated).unwrap_or(Duration::ZERO)
    }

    pub fn session_id(&self) -> Uuid {
        self.id
    }

    pub fn connection_id(&self) -> Uuid {
        self.client_id
    }

    pub fn state(&self) -> &ConnectionState {
        &self.connection_state
    }

    pub fn migration_stage(&self) -> &MigrationStage {
        self.migration_stage.as_ref().unwrap_or(&MigrationStage::Initiated)
    }

    pub fn metrics(&self) -> &ConnectionMetrics {
        &self.metrics
    }

    pub fn updated_at(&self) -> SystemTime {
        self.last_updated
    }

    pub fn retry_count(&self) -> u32 {
        self.metrics.error_count
    }

    pub fn with_state(mut self, new_state: ConnectionState) -> Self {
        self.connection_state = new_state;
        self.last_updated = SystemTime::now();
        self
    }

    pub fn with_migration_stage(mut self, new_stage: MigrationStage) -> Self {
        self.migration_stage = Some(new_stage);
        self.last_updated = SystemTime::now();
        self
    }

    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.metrics.error_count = count;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_connection_session() {
        let connection_id = Uuid::new_v4();
        let session = ConnectionSession::new(connection_id);

        assert_eq!(session.client_id, connection_id);
        assert_eq!(session.connection_state, ConnectionState::Active);
        assert_eq!(session.migration_stage, None);
        assert!(session.can_migrate());
    }

    #[test]
    fn test_migration_workflow() {
        let connection_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(connection_id, source_id);

        // Initiate migration
        session.initiate_migration(target_id, "token123".to_string());
        assert!(session.is_migrating());
        assert_eq!(session.migration_stage, Some(MigrationStage::Initiated));

        // Confirm migration
        session.confirm_migration();
        assert_eq!(session.migration_stage, Some(MigrationStage::Confirmed));

        // Start state transfer
        let snapshot_id = Uuid::new_v4();
        session.start_state_transfer(snapshot_id);
        assert_eq!(session.migration_stage, Some(MigrationStage::TransferringState));

        // Finalize migration
        session.finalize_migration();
        assert_eq!(session.migration_stage, Some(MigrationStage::Finalizing));

        // Complete migration
        session.complete_migration();
        assert!(session.is_completed());
        assert_eq!(session.source_server_id, target_id); // Server ID should be updated
        assert_eq!(session.target_server_id, None);
    }

    #[test]
    fn test_rollback_migration() {
        let connection_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(connection_id, source_id);

        session.initiate_migration(target_id, "token123".to_string());
        session.rollback_migration();

        assert_eq!(session.connection_state, ConnectionState::RolledBack);
        assert_eq!(session.migration_stage, None);
        assert_eq!(session.target_server_id, None);
        assert!(session.can_migrate()); // Should be able to migrate again after rollback
    }

    #[test]
    fn test_metrics_update() {
        let connection_id = Uuid::new_v4();
        let mut session = ConnectionSession::new(connection_id);

        session.update_metrics(1024, Duration::from_millis(50));
        assert_eq!(session.metrics.bytes_transferred, 1024);
        assert_eq!(session.metrics.requests_served, 1);
        assert_eq!(session.metrics.average_latency, Duration::from_millis(50));

        session.update_metrics(2048, Duration::from_millis(100));
        assert_eq!(session.metrics.bytes_transferred, 3072);
        assert_eq!(session.metrics.requests_served, 2);
        assert_eq!(session.metrics.average_latency, Duration::from_millis(75));
    }

    #[test]
    fn test_fail_migration() {
        let client_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(client_id, source_id);

        // Start migration
        session.initiate_migration(target_id, "token123".to_string());
        assert!(session.is_migrating());

        // Fail migration
        session.fail_migration("Test failure");
        assert_eq!(session.connection_state, ConnectionState::Failed);
        assert_eq!(session.migration_stage, Some(MigrationStage::Failed));
        assert_eq!(session.target_server_id, None);
        assert_eq!(session.metadata.get("error"), Some(&"Test failure".to_string()));
        assert!(!session.can_migrate()); // Should not be able to migrate after failure
    }

    #[test]
    fn test_increment_error_count() {
        let client_id = Uuid::new_v4();
        let server_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(client_id, server_id);

        // Initially no errors
        assert_eq!(session.metrics.error_count, 0);

        // Increment error count
        session.increment_error_count();
        assert_eq!(session.metrics.error_count, 1);

        session.increment_error_count();
        assert_eq!(session.metrics.error_count, 2);

        // Update metrics should not reset error count
        session.update_metrics(1024, Duration::from_millis(50));
        assert_eq!(session.metrics.error_count, 2);
    }

    #[test]
    fn test_age() {
        let client_id = Uuid::new_v4();
        let server_id = Uuid::new_v4();
        let session = ConnectionSession::with_servers(client_id, server_id);

        // Age should be very small (just created)
        let age = session.age();
        assert!(age < Duration::from_secs(1)); // Should be less than 1 second

        // Wait a bit and check age again
        std::thread::sleep(Duration::from_millis(10));
        let age_after_wait = session.age();
        assert!(age_after_wait > age); // Should be older
        assert!(age_after_wait >= Duration::from_millis(10)); // At least 10ms old
    }

    #[test]
    fn test_time_since_last_update() {
        let client_id = Uuid::new_v4();
        let server_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(client_id, server_id);

        // Initially, last_updated should be very recent
        let initial_time_since_update = session.time_since_last_update();
        assert!(initial_time_since_update < Duration::from_secs(1));

        // Wait a bit
        std::thread::sleep(Duration::from_millis(10));
        let time_after_wait = session.time_since_last_update();
        assert!(time_after_wait > initial_time_since_update);
        assert!(time_after_wait >= Duration::from_millis(10));

        // Update metrics should reset the last_updated time
        session.update_metrics(1024, Duration::from_millis(50));
        let time_after_update = session.time_since_last_update();
        assert!(time_after_update < time_after_wait); // Should be much smaller after update
        assert!(time_after_update < Duration::from_millis(5)); // Should be very recent
    }

    #[test]
    fn test_fail_migration_from_different_stages() {
        let client_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(client_id, source_id);

        // Test failing from confirmed stage
        session.initiate_migration(target_id, "token123".to_string());
        session.confirm_migration();
        assert_eq!(session.migration_stage, Some(MigrationStage::Confirmed));

        session.fail_migration("Confirmed stage failure");
        assert_eq!(session.connection_state, ConnectionState::Failed);
        assert_eq!(session.migration_stage, Some(MigrationStage::Failed));

        // Reset for next test
        let mut session2 = ConnectionSession::with_servers(client_id, source_id);
        
        // Test failing from state transfer stage
        session2.initiate_migration(target_id, "token456".to_string());
        session2.confirm_migration();
        let snapshot_id = Uuid::new_v4();
        session2.start_state_transfer(snapshot_id);
        assert_eq!(session2.migration_stage, Some(MigrationStage::TransferringState));

        session2.fail_migration("State transfer failure");
        assert_eq!(session2.connection_state, ConnectionState::Failed);
        assert_eq!(session2.migration_stage, Some(MigrationStage::Failed));
    }

    #[test]
    fn test_age_and_time_since_update_precision() {
        let client_id = Uuid::new_v4();
        let server_id = Uuid::new_v4();
        let mut session = ConnectionSession::with_servers(client_id, server_id);

        // Both age and time_since_last_update should be approximately equal initially
        let age1 = session.age();
        let time_since_update1 = session.time_since_last_update();
        
        // They should be within a small margin of each other (both measuring from creation)
        let diff = if age1 > time_since_update1 {
            age1 - time_since_update1
        } else {
            time_since_update1 - age1
        };
        assert!(diff < Duration::from_millis(5)); // Should be very close

        // After an update, age should continue growing but time_since_last_update resets
        std::thread::sleep(Duration::from_millis(20));
        session.update_metrics(1024, Duration::from_millis(50));
        
        let age2 = session.age();
        let time_since_update2 = session.time_since_last_update();
        
        // Age should be larger (measures from creation)
        assert!(age2 > time_since_update2);
        assert!(age2 >= Duration::from_millis(20)); // At least 20ms old
        assert!(time_since_update2 < Duration::from_millis(5)); // Very recent update
    }
}