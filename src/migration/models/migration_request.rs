use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationReason {
    PlannedMaintenance,
    LoadBalancing,
    HealthDegradation,
    AdminRequest,
    UserRequested,
    Emergency,
}

impl Default for MigrationReason {
    fn default() -> Self {
        MigrationReason::AdminRequest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
    Emergency = 5,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInstance {
    pub id: Uuid,
    pub address: String,
    pub management_port: u16,
    pub userspace_port: u16,
    pub weight: f32,
    pub capacity: u32,
    pub current_connections: u32,
    pub metadata: HashMap<String, String>,
}

impl ServerInstance {
    pub fn new(
        address: String,
        management_port: u16,
        userspace_port: u16,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            address,
            management_port,
            userspace_port,
            weight: 1.0,
            capacity: 1000,
            current_connections: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn load_percentage(&self) -> f32 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current_connections as f32 / self.capacity as f32) * 100.0
        }
    }

    pub fn available_capacity(&self) -> u32 {
        self.capacity.saturating_sub(self.current_connections)
    }

    pub fn can_accept_connections(&self) -> bool {
        self.current_connections < self.capacity
    }

    pub fn management_endpoint(&self) -> String {
        format!("{}:{}", self.address, self.management_port)
    }

    pub fn userspace_endpoint(&self) -> String {
        format!("{}:{}", self.address, self.userspace_port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRequest {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub source_server: ServerInstance,
    pub target_server: ServerInstance,
    pub reason: MigrationReason,
    pub priority: Priority,
    pub requested_by: String,
    pub requested_at: SystemTime,
    pub deadline: Option<SystemTime>,
    pub preserve_session: bool,
    pub require_acknowledgment: bool,
    pub timeout_seconds: u32,
    pub metadata: HashMap<String, String>,
}

impl MigrationRequest {
    pub fn new(
        connection_id: Uuid,
        source_server: ServerInstance,
        target_server: ServerInstance,
        reason: MigrationReason,
        requested_by: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            connection_id,
            source_server,
            target_server,
            reason,
            priority: Priority::default(),
            requested_by,
            requested_at: SystemTime::now(),
            deadline: None,
            preserve_session: true,
            require_acknowledgment: true,
            timeout_seconds: 30,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_deadline(mut self, deadline: SystemTime) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u32) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn no_session_preservation(mut self) -> Self {
        self.preserve_session = false;
        self
    }

    pub fn no_acknowledgment(mut self) -> Self {
        self.require_acknowledgment = false;
        self
    }

    pub fn is_high_priority(&self) -> bool {
        matches!(
            self.priority,
            Priority::High | Priority::Critical | Priority::Emergency
        )
    }

    pub fn is_emergency(&self) -> bool {
        matches!(self.priority, Priority::Emergency) || 
        matches!(self.reason, MigrationReason::Emergency)
    }

    pub fn is_overdue(&self) -> bool {
        if let Some(deadline) = self.deadline {
            SystemTime::now() > deadline
        } else {
            false
        }
    }

    pub fn time_to_deadline(&self) -> Option<std::time::Duration> {
        self.deadline.and_then(|deadline| {
            deadline.duration_since(SystemTime::now()).ok()
        })
    }

    pub fn age(&self) -> std::time::Duration {
        SystemTime::now()
            .duration_since(self.requested_at)
            .unwrap_or(std::time::Duration::ZERO)
    }

    pub fn should_timeout(&self) -> bool {
        self.age().as_secs() > self.timeout_seconds as u64
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.source_server.id == self.target_server.id {
            return Err("Source and target servers cannot be the same".to_string());
        }

        if !self.target_server.can_accept_connections() {
            return Err("Target server is at capacity".to_string());
        }

        if self.timeout_seconds == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        if let Some(deadline) = self.deadline {
            if deadline <= self.requested_at {
                return Err("Deadline must be in the future".to_string());
            }
        }

        Ok(())
    }

    pub fn priority_score(&self) -> u8 {
        let base_score = self.priority.clone() as u8;
        let urgency_bonus = if self.is_overdue() { 2 } else { 0 };
        let age_bonus = (self.age().as_secs() / 60).min(5) as u8; // Max 5 points for age
        
        (base_score + urgency_bonus + age_bonus).min(255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_server_instance_creation() {
        let server = ServerInstance::new(
            "127.0.0.1".to_string(),
            8081,
            8080,
        );

        assert_eq!(server.address, "127.0.0.1");
        assert_eq!(server.management_port, 8081);
        assert_eq!(server.userspace_port, 8080);
        assert_eq!(server.weight, 1.0);
        assert_eq!(server.capacity, 1000);
        assert_eq!(server.current_connections, 0);
        assert!(server.can_accept_connections());
        assert_eq!(server.load_percentage(), 0.0);
    }

    #[test]
    fn test_server_instance_load_calculation() {
        let mut server = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        server.capacity = 100;
        server.current_connections = 25;

        assert_eq!(server.load_percentage(), 25.0);
        assert_eq!(server.available_capacity(), 75);
        assert!(server.can_accept_connections());

        server.current_connections = 100;
        assert_eq!(server.load_percentage(), 100.0);
        assert_eq!(server.available_capacity(), 0);
        assert!(!server.can_accept_connections());
    }

    #[test]
    fn test_server_instance_endpoints() {
        let server = ServerInstance::new("example.com".to_string(), 9091, 9090);
        assert_eq!(server.management_endpoint(), "example.com:9091");
        assert_eq!(server.userspace_endpoint(), "example.com:9090");
    }

    #[test]
    fn test_migration_request_creation() {
        let connection_id = Uuid::new_v4();
        let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);

        let request = MigrationRequest::new(
            connection_id,
            source.clone(),
            target.clone(),
            MigrationReason::PlannedMaintenance,
            "admin".to_string(),
        );

        assert_eq!(request.connection_id, connection_id);
        assert_eq!(request.source_server.id, source.id);
        assert_eq!(request.target_server.id, target.id);
        assert_eq!(request.reason, MigrationReason::PlannedMaintenance);
        assert_eq!(request.priority, Priority::Normal);
        assert!(request.preserve_session);
        assert!(request.require_acknowledgment);
        assert_eq!(request.timeout_seconds, 30);
    }

    #[test]
    fn test_migration_request_builder_pattern() {
        let connection_id = Uuid::new_v4();
        let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);

        let deadline = SystemTime::now() + Duration::from_secs(3600);
        let request = MigrationRequest::new(
            connection_id,
            source,
            target,
            MigrationReason::Emergency,
            "system".to_string(),
        )
        .with_priority(Priority::Critical)
        .with_deadline(deadline)
        .with_timeout(60)
        .with_metadata("reason".to_string(), "server_failure".to_string())
        .no_session_preservation();

        assert_eq!(request.priority, Priority::Critical);
        assert_eq!(request.deadline, Some(deadline));
        assert_eq!(request.timeout_seconds, 60);
        assert!(!request.preserve_session);
        assert!(request.is_high_priority());
        assert!(request.is_emergency());
        assert_eq!(request.metadata.get("reason"), Some(&"server_failure".to_string()));
    }

    #[test]
    fn test_migration_request_validation() {
        let connection_id = Uuid::new_v4();
        let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);

        // Valid request
        let request = MigrationRequest::new(
            connection_id,
            source.clone(),
            target.clone(),
            MigrationReason::PlannedMaintenance,
            "admin".to_string(),
        );
        assert!(request.validate().is_ok());

        // Same source and target
        let mut invalid_request = request.clone();
        invalid_request.target_server.id = invalid_request.source_server.id;
        assert!(invalid_request.validate().is_err());

        // Target at capacity
        let mut target_full = target.clone();
        target_full.current_connections = target_full.capacity;
        let full_request = MigrationRequest::new(
            connection_id,
            source.clone(),
            target_full,
            MigrationReason::PlannedMaintenance,
            "admin".to_string(),
        );
        assert!(full_request.validate().is_err());

        // Zero timeout
        let zero_timeout = request.with_timeout(0);
        assert!(zero_timeout.validate().is_err());
    }

    #[test]
    fn test_migration_request_priority_scoring() {
        let connection_id = Uuid::new_v4();
        let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);

        let low_priority = MigrationRequest::new(
            connection_id,
            source.clone(),
            target.clone(),
            MigrationReason::PlannedMaintenance,
            "admin".to_string(),
        ).with_priority(Priority::Low);

        let high_priority = MigrationRequest::new(
            connection_id,
            source.clone(),
            target.clone(),
            MigrationReason::Emergency,
            "system".to_string(),
        ).with_priority(Priority::Emergency);

        assert!(high_priority.priority_score() > low_priority.priority_score());
        assert!(high_priority.is_emergency());
        assert!(!low_priority.is_emergency());
    }
}