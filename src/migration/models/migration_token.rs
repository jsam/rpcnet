use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenPurpose {
    MigrationAuthorization,
    StateTransfer,
    HealthCheck,
    ConnectionHandoff,
}

impl Default for TokenPurpose {
    fn default() -> Self {
        TokenPurpose::MigrationAuthorization
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenState {
    Generated,
    Active,
    Used,
    Expired,
    Revoked,
}

impl Default for TokenState {
    fn default() -> Self {
        TokenState::Generated
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    pub hostname: String,
    pub userspace_port: u16,
    pub management_port: u16,
}

impl ServerEndpoint {
    pub fn new(hostname: String, userspace_port: u16, management_port: u16) -> Self {
        Self {
            hostname,
            userspace_port,
            management_port,
        }
    }

    pub fn userspace_address(&self) -> String {
        format!("{}:{}", self.hostname, self.userspace_port)
    }

    pub fn management_address(&self) -> String {
        format!("{}:{}", self.hostname, self.management_port)
    }

    pub fn is_local(&self) -> bool {
        self.hostname == "127.0.0.1" || self.hostname == "localhost"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationToken {
    pub token_value: [u8; 32], // 256-bit cryptographically random token
    pub session_id: Uuid,
    pub generated_at: SystemTime,
    pub expires_at: SystemTime,
    pub used_at: Option<SystemTime>,
    pub issuer: ServerEndpoint,
    pub purpose: TokenPurpose,
    pub state: TokenState,
    pub metadata: std::collections::HashMap<String, String>,
}

impl MigrationToken {
    pub fn new(
        session_id: Uuid,
        issuer: ServerEndpoint,
        purpose: TokenPurpose,
        validity_duration: Duration,
    ) -> Self {
        let now = SystemTime::now();
        let token_value = generate_random_token();
        
        Self {
            token_value,
            session_id,
            generated_at: now,
            expires_at: now + validity_duration,
            used_at: None,
            issuer,
            purpose,
            state: TokenState::default(),
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn for_migration(session_id: Uuid, issuer: ServerEndpoint) -> Self {
        Self::new(
            session_id,
            issuer,
            TokenPurpose::MigrationAuthorization,
            Duration::from_secs(300), // 5 minutes default
        )
    }

    pub fn for_state_transfer(session_id: Uuid, issuer: ServerEndpoint) -> Self {
        Self::new(
            session_id,
            issuer,
            TokenPurpose::StateTransfer,
            Duration::from_secs(600), // 10 minutes for state transfer
        )
    }

    pub fn for_health_check(session_id: Uuid, issuer: ServerEndpoint) -> Self {
        Self::new(
            session_id,
            issuer,
            TokenPurpose::HealthCheck,
            Duration::from_secs(60), // 1 minute for health checks
        )
    }

    pub fn for_connection_handoff(session_id: Uuid, issuer: ServerEndpoint) -> Self {
        Self::new(
            session_id,
            issuer,
            TokenPurpose::ConnectionHandoff,
            Duration::from_secs(120), // 2 minutes for handoff
        )
    }

    pub fn with_expiration(mut self, expires_at: SystemTime) -> Self {
        self.expires_at = expires_at;
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn activate(&mut self) {
        if matches!(self.state, TokenState::Generated) {
            self.state = TokenState::Active;
        }
    }

    pub fn use_token(&mut self) -> Result<(), String> {
        match self.state {
            TokenState::Active => {
                if self.is_expired() {
                    self.state = TokenState::Expired;
                    Err("Token has expired".to_string())
                } else {
                    self.state = TokenState::Used;
                    self.used_at = Some(SystemTime::now());
                    Ok(())
                }
            }
            TokenState::Used => Err("Token has already been used".to_string()),
            TokenState::Expired => Err("Token has expired".to_string()),
            TokenState::Revoked => Err("Token has been revoked".to_string()),
            TokenState::Generated => Err("Token is not yet active".to_string()),
        }
    }

    pub fn revoke(&mut self) {
        self.state = TokenState::Revoked;
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    pub fn is_valid(&self) -> bool {
        matches!(self.state, TokenState::Active) && !self.is_expired()
    }

    pub fn is_used(&self) -> bool {
        matches!(self.state, TokenState::Used)
    }

    pub fn is_revoked(&self) -> bool {
        matches!(self.state, TokenState::Revoked)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, TokenState::Active)
    }

    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.generated_at)
            .unwrap_or(Duration::ZERO)
    }

    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.expires_at.duration_since(SystemTime::now()).ok()
    }

    pub fn time_since_use(&self) -> Option<Duration> {
        self.used_at.and_then(|used_at| {
            SystemTime::now().duration_since(used_at).ok()
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        // Check if token value is non-zero
        if self.token_value == [0u8; 32] {
            return Err("Token value cannot be all zeros".to_string());
        }

        // Check expiration time is reasonable
        if self.expires_at <= self.generated_at {
            return Err("Token expiration must be after generation time".to_string());
        }

        // Check if used_at is consistent with state
        if self.used_at.is_some() && !matches!(self.state, TokenState::Used) {
            return Err("Token marked as used but state is not Used".to_string());
        }

        if self.used_at.is_none() && matches!(self.state, TokenState::Used) {
            return Err("Token state is Used but no used_at timestamp".to_string());
        }

        // Check if used_at is after generated_at
        if let Some(used_at) = self.used_at {
            if used_at < self.generated_at {
                return Err("Token used before it was generated".to_string());
            }
        }

        // Check issuer endpoint validity
        if self.issuer.hostname.is_empty() {
            return Err("Issuer hostname cannot be empty".to_string());
        }

        if self.issuer.userspace_port == 0 || self.issuer.management_port == 0 {
            return Err("Issuer ports must be non-zero".to_string());
        }

        if self.issuer.userspace_port == self.issuer.management_port {
            return Err("Userspace and management ports must be different".to_string());
        }

        Ok(())
    }

    pub fn token_hex(&self) -> String {
        hex::encode(self.token_value)
    }

    pub fn from_hex(hex_str: &str) -> Result<[u8; 32], String> {
        if hex_str.len() != 64 {
            return Err("Token hex string must be exactly 64 characters".to_string());
        }

        let bytes = hex::decode(hex_str)
            .map_err(|e| format!("Invalid hex string: {}", e))?;

        if bytes.len() != 32 {
            return Err("Token must be exactly 32 bytes".to_string());
        }

        let mut token_value = [0u8; 32];
        token_value.copy_from_slice(&bytes);
        Ok(token_value)
    }

    pub fn matches_session(&self, session_id: Uuid) -> bool {
        self.session_id == session_id
    }

    pub fn matches_purpose(&self, purpose: &TokenPurpose) -> bool {
        &self.purpose == purpose
    }

    pub fn matches_issuer(&self, issuer: &ServerEndpoint) -> bool {
        self.issuer.hostname == issuer.hostname
            && self.issuer.userspace_port == issuer.userspace_port
            && self.issuer.management_port == issuer.management_port
    }

    pub fn summary(&self) -> String {
        let state_str = match self.state {
            TokenState::Generated => "generated",
            TokenState::Active => "active",
            TokenState::Used => "used",
            TokenState::Expired => "expired",
            TokenState::Revoked => "revoked",
        };

        let purpose_str = match self.purpose {
            TokenPurpose::MigrationAuthorization => "migration",
            TokenPurpose::StateTransfer => "state_transfer",
            TokenPurpose::HealthCheck => "health_check",
            TokenPurpose::ConnectionHandoff => "handoff",
        };

        let age_str = format!("{}s", self.age().as_secs());
        let expires_str = if let Some(time_left) = self.time_until_expiry() {
            format!("expires in {}s", time_left.as_secs())
        } else {
            "expired".to_string()
        };

        format!(
            "{} {} token ({}), age: {}, {}",
            state_str,
            purpose_str,
            &self.token_hex()[..8], // First 8 hex chars for identification
            age_str,
            expires_str
        )
    }
}

// Generate a cryptographically secure random 32-byte token
fn generate_random_token() -> [u8; 32] {
    use rand::{RngCore, thread_rng};
    let mut token = [0u8; 32];
    thread_rng().fill_bytes(&mut token);
    token
}

// Implement PartialEq for MigrationToken (excluding generated_at for comparison tolerance)
impl PartialEq for MigrationToken {
    fn eq(&self, other: &Self) -> bool {
        self.token_value == other.token_value
            && self.session_id == other.session_id
            && self.purpose == other.purpose
            && self.state == other.state
    }
}

impl Eq for MigrationToken {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_migration_token() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let purpose = TokenPurpose::MigrationAuthorization;
        let validity = Duration::from_secs(300);

        let token = MigrationToken::new(session_id, issuer.clone(), purpose, validity);

        assert_eq!(token.session_id, session_id);
        assert_eq!(token.purpose, TokenPurpose::MigrationAuthorization);
        assert_eq!(token.state, TokenState::Generated);
        assert!(token.matches_issuer(&issuer));
        assert_ne!(token.token_value, [0u8; 32]); // Should be random
    }

    #[test]
    fn test_factory_methods() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("example.com".to_string(), 9090, 9091);

        let migration_token = MigrationToken::for_migration(session_id, issuer.clone());
        assert!(migration_token.matches_purpose(&TokenPurpose::MigrationAuthorization));

        let state_token = MigrationToken::for_state_transfer(session_id, issuer.clone());
        assert!(state_token.matches_purpose(&TokenPurpose::StateTransfer));

        let health_token = MigrationToken::for_health_check(session_id, issuer.clone());
        assert!(health_token.matches_purpose(&TokenPurpose::HealthCheck));

        let handoff_token = MigrationToken::for_connection_handoff(session_id, issuer);
        assert!(handoff_token.matches_purpose(&TokenPurpose::ConnectionHandoff));
    }

    #[test]
    fn test_token_lifecycle() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("localhost".to_string(), 8080, 8081);
        let mut token = MigrationToken::for_migration(session_id, issuer);

        // Initially generated
        assert_eq!(token.state, TokenState::Generated);
        assert!(!token.is_active());
        assert!(!token.is_valid());

        // Activate token
        token.activate();
        assert_eq!(token.state, TokenState::Active);
        assert!(token.is_active());
        assert!(token.is_valid());

        // Use token
        assert!(token.use_token().is_ok());
        assert_eq!(token.state, TokenState::Used);
        assert!(token.is_used());
        assert!(!token.is_valid());
        assert!(token.used_at.is_some());

        // Cannot use again
        assert!(token.use_token().is_err());
    }

    #[test]
    fn test_token_expiration() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        
        // Create expired token
        let past_time = SystemTime::now() - Duration::from_secs(60);
        let mut expired_token = MigrationToken::new(
            session_id,
            issuer,
            TokenPurpose::MigrationAuthorization,
            Duration::from_secs(30),
        ).with_expiration(past_time);

        expired_token.activate();
        assert!(expired_token.is_expired());
        assert!(!expired_token.is_valid());
        assert!(expired_token.use_token().is_err());
        assert_eq!(expired_token.state, TokenState::Expired);
    }

    #[test]
    fn test_token_revocation() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let mut token = MigrationToken::for_migration(session_id, issuer);

        token.activate();
        assert!(token.is_valid());

        token.revoke();
        assert!(token.is_revoked());
        assert!(!token.is_valid());
        assert!(token.use_token().is_err());
    }

    #[test]
    fn test_token_validation() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("example.com".to_string(), 8080, 8081);

        // Valid token
        let valid_token = MigrationToken::for_migration(session_id, issuer.clone());
        assert!(valid_token.validate().is_ok());

        // Invalid token with zero value
        let mut invalid_token = valid_token.clone();
        invalid_token.token_value = [0u8; 32];
        assert!(invalid_token.validate().is_err());

        // Invalid token with bad expiration
        let mut bad_expiry = MigrationToken::for_migration(session_id, issuer.clone());
        bad_expiry.expires_at = bad_expiry.generated_at - Duration::from_secs(10);
        assert!(bad_expiry.validate().is_err());

        // Invalid issuer
        let bad_issuer = ServerEndpoint::new("".to_string(), 0, 0);
        let bad_issuer_token = MigrationToken::for_migration(session_id, bad_issuer);
        assert!(bad_issuer_token.validate().is_err());
    }

    #[test]
    fn test_hex_conversion() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let token = MigrationToken::for_migration(session_id, issuer);

        let hex_str = token.token_hex();
        assert_eq!(hex_str.len(), 64); // 32 bytes = 64 hex chars

        let recovered_token_value = MigrationToken::from_hex(&hex_str).unwrap();
        assert_eq!(recovered_token_value, token.token_value);

        // Test invalid hex
        assert!(MigrationToken::from_hex("invalid").is_err());
        assert!(MigrationToken::from_hex("00112233").is_err()); // Too short
    }

    #[test]
    fn test_server_endpoint() {
        let endpoint = ServerEndpoint::new("example.com".to_string(), 8080, 9090);

        assert_eq!(endpoint.userspace_address(), "example.com:8080");
        assert_eq!(endpoint.management_address(), "example.com:9090");
        assert!(!endpoint.is_local());

        let local_endpoint = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 9090);
        assert!(local_endpoint.is_local());

        let localhost_endpoint = ServerEndpoint::new("localhost".to_string(), 8080, 9090);
        assert!(localhost_endpoint.is_local());
    }

    #[test]
    fn test_token_matching() {
        let session_id = Uuid::new_v4();
        let other_session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("example.com".to_string(), 8080, 8081);
        let other_issuer = ServerEndpoint::new("other.com".to_string(), 8080, 8081);

        let token = MigrationToken::for_migration(session_id, issuer.clone());

        // Session matching
        assert!(token.matches_session(session_id));
        assert!(!token.matches_session(other_session_id));

        // Purpose matching
        assert!(token.matches_purpose(&TokenPurpose::MigrationAuthorization));
        assert!(!token.matches_purpose(&TokenPurpose::StateTransfer));

        // Issuer matching
        assert!(token.matches_issuer(&issuer));
        assert!(!token.matches_issuer(&other_issuer));
    }

    #[test]
    fn test_token_metadata() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);

        let token = MigrationToken::for_migration(session_id, issuer)
            .with_metadata("client_id".to_string(), "client-123".to_string())
            .with_metadata("priority".to_string(), "high".to_string());

        assert_eq!(token.metadata.get("client_id"), Some(&"client-123".to_string()));
        assert_eq!(token.metadata.get("priority"), Some(&"high".to_string()));
    }

    #[test]
    fn test_token_summary() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let token = MigrationToken::for_migration(session_id, issuer);

        let summary = token.summary();
        assert!(summary.contains("generated"));
        assert!(summary.contains("migration"));
        assert!(summary.contains("expires in"));
    }

    #[test]
    fn test_time_calculations() {
        let session_id = Uuid::new_v4();
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let mut token = MigrationToken::for_migration(session_id, issuer);

        // Age should be very small initially
        assert!(token.age() < Duration::from_secs(1));

        // Time until expiry should be close to original duration
        let time_until_expiry = token.time_until_expiry().unwrap();
        assert!(time_until_expiry > Duration::from_secs(290)); // Close to 5 minutes

        // Time since use should be None initially
        assert!(token.time_since_use().is_none());

        // After using the token
        token.activate();
        token.use_token().unwrap();
        assert!(token.time_since_use().is_some());
        assert!(token.time_since_use().unwrap() < Duration::from_secs(1));
    }
}