use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::migration::models::migration_token::{MigrationToken, TokenType, TokenStatus};

#[derive(Debug, Clone)]
pub struct TokenValidationConfig {
    pub max_age: Duration,
    pub allow_expired_grace_period: Duration,
    pub rate_limit_window: Duration,
    pub max_tokens_per_window: usize,
    pub require_source_verification: bool,
}

impl Default for TokenValidationConfig {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(300), // 5 minutes
            allow_expired_grace_period: Duration::from_secs(30), // 30 seconds
            rate_limit_window: Duration::from_secs(60), // 1 minute
            max_tokens_per_window: 10,
            require_source_verification: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenValidationError {
    TokenNotFound,
    TokenExpired,
    TokenAlreadyUsed,
    TokenInvalidStatus,
    TokenTypeNotSupported,
    SourceMismatch,
    TargetMismatch,
    SessionMismatch,
    RateLimitExceeded,
    InvalidTokenFormat,
    InvalidTimestamp,
    TokenRevoked,
}

impl std::fmt::Display for TokenValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenValidationError::TokenNotFound => write!(f, "Token not found"),
            TokenValidationError::TokenExpired => write!(f, "Token has expired"),
            TokenValidationError::TokenAlreadyUsed => write!(f, "Token has already been used"),
            TokenValidationError::TokenInvalidStatus => write!(f, "Token has invalid status"),
            TokenValidationError::TokenTypeNotSupported => write!(f, "Token type not supported"),
            TokenValidationError::SourceMismatch => write!(f, "Source server mismatch"),
            TokenValidationError::TargetMismatch => write!(f, "Target server mismatch"),
            TokenValidationError::SessionMismatch => write!(f, "Session ID mismatch"),
            TokenValidationError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            TokenValidationError::InvalidTokenFormat => write!(f, "Invalid token format"),
            TokenValidationError::InvalidTimestamp => write!(f, "Invalid timestamp"),
            TokenValidationError::TokenRevoked => write!(f, "Token has been revoked"),
        }
    }
}

impl std::error::Error for TokenValidationError {}

#[derive(Debug)]
struct TokenUsageRecord {
    count: usize,
    last_used: SystemTime,
    first_used: SystemTime,
}

impl TokenUsageRecord {
    fn new() -> Self {
        let now = SystemTime::now();
        Self {
            count: 1,
            last_used: now,
            first_used: now,
        }
    }

    fn increment(&mut self) {
        self.count += 1;
        self.last_used = SystemTime::now();
    }

    fn is_within_window(&self, window: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.first_used)
            .unwrap_or(Duration::MAX) <= window
    }
}

pub struct MigrationTokenManager {
    config: TokenValidationConfig,
    active_tokens: HashMap<String, MigrationToken>,
    usage_tracking: HashMap<Uuid, TokenUsageRecord>, // Source server -> usage
    revoked_tokens: HashMap<String, SystemTime>,
}

impl MigrationTokenManager {
    pub fn new(config: TokenValidationConfig) -> Self {
        Self {
            config,
            active_tokens: HashMap::new(),
            usage_tracking: HashMap::new(),
            revoked_tokens: HashMap::new(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(TokenValidationConfig::default())
    }

    pub fn generate_token(
        &mut self,
        session_id: Uuid,
        source_server: Uuid,
        target_server: Uuid,
        token_type: TokenType,
    ) -> Result<MigrationToken, String> {
        // Clean up old tokens before generating new ones
        self.cleanup_expired_tokens();

        // Check rate limiting
        if let Err(e) = self.check_rate_limit(source_server) {
            return Err(format!("Rate limit check failed: {}", e));
        }

        let token = MigrationToken::new(
            session_id,
            source_server,
            target_server,
            token_type,
        );

        // Store the token
        self.active_tokens.insert(token.token_value.clone(), token.clone());

        // Update usage tracking
        self.usage_tracking
            .entry(source_server)
            .and_modify(|record| record.increment())
            .or_insert_with(TokenUsageRecord::new);

        Ok(token)
    }

    pub fn validate_token(
        &self,
        token_value: &str,
        expected_session_id: Option<Uuid>,
        expected_source: Option<Uuid>,
        expected_target: Option<Uuid>,
    ) -> Result<&MigrationToken, TokenValidationError> {
        // Check if token is revoked
        if self.revoked_tokens.contains_key(token_value) {
            return Err(TokenValidationError::TokenRevoked);
        }

        // Find the token
        let token = self.active_tokens.get(token_value)
            .ok_or(TokenValidationError::TokenNotFound)?;

        // Validate token status
        if token.status != TokenStatus::Active {
            return match token.status {
                TokenStatus::Used => Err(TokenValidationError::TokenAlreadyUsed),
                TokenStatus::Expired => Err(TokenValidationError::TokenExpired),
                TokenStatus::Revoked => Err(TokenValidationError::TokenRevoked),
                _ => Err(TokenValidationError::TokenInvalidStatus),
            };
        }

        // Check expiration with grace period
        if self.is_token_expired(token) {
            return Err(TokenValidationError::TokenExpired);
        }

        // Validate session ID if provided
        if let Some(session_id) = expected_session_id {
            if token.session_id != session_id {
                return Err(TokenValidationError::SessionMismatch);
            }
        }

        // Validate source server if required
        if self.config.require_source_verification {
            if let Some(source) = expected_source {
                if token.source_server != source {
                    return Err(TokenValidationError::SourceMismatch);
                }
            }
        }

        // Validate target server if provided
        if let Some(target) = expected_target {
            if token.target_server != target {
                return Err(TokenValidationError::TargetMismatch);
            }
        }

        Ok(token)
    }

    pub fn consume_token(
        &mut self,
        token_value: &str,
        expected_session_id: Option<Uuid>,
        expected_source: Option<Uuid>,
        expected_target: Option<Uuid>,
    ) -> Result<MigrationToken, TokenValidationError> {
        // First validate the token
        self.validate_token(token_value, expected_session_id, expected_source, expected_target)?;

        // Remove and return the token, marking it as used
        let mut token = self.active_tokens.remove(token_value)
            .ok_or(TokenValidationError::TokenNotFound)?;

        token.mark_used();
        Ok(token)
    }

    pub fn revoke_token(&mut self, token_value: &str) -> Result<(), TokenValidationError> {
        if let Some(mut token) = self.active_tokens.remove(token_value) {
            token.revoke();
            self.revoked_tokens.insert(token_value.to_string(), SystemTime::now());
        }
        Ok(())
    }

    pub fn revoke_session_tokens(&mut self, session_id: Uuid) -> usize {
        let mut revoked_count = 0;
        let tokens_to_revoke: Vec<String> = self.active_tokens
            .iter()
            .filter(|(_, token)| token.session_id == session_id)
            .map(|(key, _)| key.clone())
            .collect();

        for token_value in tokens_to_revoke {
            if self.revoke_token(&token_value).is_ok() {
                revoked_count += 1;
            }
        }

        revoked_count
    }

    pub fn cleanup_expired_tokens(&mut self) -> usize {
        let now = SystemTime::now();
        let mut removed_count = 0;

        // Remove expired tokens
        self.active_tokens.retain(|token_value, token| {
            if self.is_token_expired(token) {
                self.revoked_tokens.insert(token_value.clone(), now);
                removed_count += 1;
                false
            } else {
                true
            }
        });

        // Clean up old revoked token records (keep for 24 hours)
        let cleanup_threshold = Duration::from_secs(86400); // 24 hours
        self.revoked_tokens.retain(|_, revoked_time| {
            now.duration_since(*revoked_time).unwrap_or(Duration::ZERO) < cleanup_threshold
        });

        // Clean up old usage records
        self.usage_tracking.retain(|_, record| {
            now.duration_since(record.last_used).unwrap_or(Duration::ZERO) < cleanup_threshold
        });

        removed_count
    }

    pub fn get_active_token_count(&self) -> usize {
        self.active_tokens.len()
    }

    pub fn get_tokens_for_session(&self, session_id: Uuid) -> Vec<&MigrationToken> {
        self.active_tokens
            .values()
            .filter(|token| token.session_id == session_id)
            .collect()
    }

    pub fn get_usage_stats(&self, source_server: Uuid) -> Option<(usize, SystemTime, SystemTime)> {
        self.usage_tracking.get(&source_server)
            .map(|record| (record.count, record.first_used, record.last_used))
    }

    fn is_token_expired(&self, token: &MigrationToken) -> bool {
        let now = SystemTime::now();
        let age = now.duration_since(token.created_at).unwrap_or(Duration::ZERO);
        
        if age > self.config.max_age {
            // Check if within grace period
            let grace_limit = self.config.max_age + self.config.allow_expired_grace_period;
            age > grace_limit
        } else {
            false
        }
    }

    fn check_rate_limit(&self, source_server: Uuid) -> Result<(), TokenValidationError> {
        if let Some(record) = self.usage_tracking.get(&source_server) {
            if record.is_within_window(self.config.rate_limit_window) 
                && record.count >= self.config.max_tokens_per_window {
                return Err(TokenValidationError::RateLimitExceeded);
            }
        }
        Ok(())
    }

    pub fn update_config(&mut self, config: TokenValidationConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &TokenValidationConfig {
        &self.config
    }
}

pub mod utils {
    use super::*;

    pub fn parse_token_from_header(header_value: &str) -> Result<String, TokenValidationError> {
        if header_value.starts_with("Bearer ") {
            let token = header_value.strip_prefix("Bearer ").unwrap_or("");
            if token.is_empty() {
                Err(TokenValidationError::InvalidTokenFormat)
            } else {
                Ok(token.to_string())
            }
        } else {
            Err(TokenValidationError::InvalidTokenFormat)
        }
    }

    pub fn create_auth_header(token: &str) -> String {
        format!("Bearer {}", token)
    }

    pub fn validate_token_format(token_value: &str) -> Result<(), TokenValidationError> {
        // Check hex format (64 characters)
        if token_value.len() != 64 {
            return Err(TokenValidationError::InvalidTokenFormat);
        }

        if !token_value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TokenValidationError::InvalidTokenFormat);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> TokenValidationConfig {
        TokenValidationConfig {
            max_age: Duration::from_secs(60),
            allow_expired_grace_period: Duration::from_secs(10),
            rate_limit_window: Duration::from_secs(30),
            max_tokens_per_window: 3,
            require_source_verification: true,
        }
    }

    #[test]
    fn test_token_generation() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let token = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        ).unwrap();

        assert_eq!(token.session_id, session_id);
        assert_eq!(token.source_server, source);
        assert_eq!(token.target_server, target);
        assert_eq!(token.token_type, TokenType::StateTransfer);
        assert_eq!(token.status, TokenStatus::Active);
        assert_eq!(manager.get_active_token_count(), 1);
    }

    #[test]
    fn test_token_validation_success() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let token = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        ).unwrap();

        let validated = manager.validate_token(
            &token.token_value,
            Some(session_id),
            Some(source),
            Some(target),
        ).unwrap();

        assert_eq!(validated.session_id, session_id);
    }

    #[test]
    fn test_token_validation_failure() {
        let manager = MigrationTokenManager::new(create_test_config());
        
        let result = manager.validate_token(
            "nonexistent_token",
            None,
            None,
            None,
        );

        assert!(matches!(result, Err(TokenValidationError::TokenNotFound)));
    }

    #[test]
    fn test_token_consumption() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let token = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        ).unwrap();

        let consumed = manager.consume_token(
            &token.token_value,
            Some(session_id),
            Some(source),
            Some(target),
        ).unwrap();

        assert_eq!(consumed.status, TokenStatus::Used);
        assert_eq!(manager.get_active_token_count(), 0);

        // Try to use the same token again
        let result = manager.validate_token(
            &token.token_value,
            Some(session_id),
            Some(source),
            Some(target),
        );
        assert!(matches!(result, Err(TokenValidationError::TokenNotFound)));
    }

    #[test]
    fn test_rate_limiting() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // Generate maximum allowed tokens
        for _ in 0..3 {
            let _token = manager.generate_token(
                Uuid::new_v4(), // Different session IDs
                source,
                target,
                TokenType::StateTransfer,
            ).unwrap();
        }

        // This should fail due to rate limiting
        let result = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Rate limit"));
    }

    #[test]
    fn test_token_revocation() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let token = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        ).unwrap();

        // Revoke the token
        manager.revoke_token(&token.token_value).unwrap();

        // Validation should fail
        let result = manager.validate_token(
            &token.token_value,
            Some(session_id),
            Some(source),
            Some(target),
        );

        assert!(matches!(result, Err(TokenValidationError::TokenRevoked)));
    }

    #[test]
    fn test_session_token_revocation() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // Generate multiple tokens for the same session
        for _ in 0..3 {
            let _token = manager.generate_token(
                session_id,
                Uuid::new_v4(), // Different sources to avoid rate limiting
                target,
                TokenType::StateTransfer,
            ).unwrap();
        }

        assert_eq!(manager.get_active_token_count(), 3);

        // Revoke all tokens for the session
        let revoked_count = manager.revoke_session_tokens(session_id);
        assert_eq!(revoked_count, 3);
        assert_eq!(manager.get_active_token_count(), 0);
    }

    #[test]
    fn test_cleanup_expired_tokens() {
        let config = TokenValidationConfig {
            max_age: Duration::from_millis(1), // Very short expiration
            allow_expired_grace_period: Duration::from_millis(1),
            rate_limit_window: Duration::from_secs(30),
            max_tokens_per_window: 10,
            require_source_verification: true,
        };

        let mut manager = MigrationTokenManager::new(config);
        let session_id = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let _token = manager.generate_token(
            session_id,
            source,
            target,
            TokenType::StateTransfer,
        ).unwrap();

        assert_eq!(manager.get_active_token_count(), 1);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Cleanup should remove the expired token
        let removed_count = manager.cleanup_expired_tokens();
        assert_eq!(removed_count, 1);
        assert_eq!(manager.get_active_token_count(), 0);
    }

    #[test]
    fn test_utils_token_parsing() {
        let header = "Bearer abc123def456";
        let token = utils::parse_token_from_header(header).unwrap();
        assert_eq!(token, "abc123def456");

        let invalid_header = "Invalid format";
        let result = utils::parse_token_from_header(invalid_header);
        assert!(matches!(result, Err(TokenValidationError::InvalidTokenFormat)));
    }

    #[test]
    fn test_utils_auth_header_creation() {
        let token = "abc123def456";
        let header = utils::create_auth_header(token);
        assert_eq!(header, "Bearer abc123def456");
    }

    #[test]
    fn test_utils_token_format_validation() {
        // Valid 64-character hex string
        let valid_token = "a".repeat(64);
        assert!(utils::validate_token_format(&valid_token).is_ok());

        // Invalid length
        let invalid_length = "a".repeat(32);
        assert!(matches!(
            utils::validate_token_format(&invalid_length),
            Err(TokenValidationError::InvalidTokenFormat)
        ));

        // Invalid characters
        let invalid_chars = "g".repeat(64);
        assert!(matches!(
            utils::validate_token_format(&invalid_chars),
            Err(TokenValidationError::InvalidTokenFormat)
        ));
    }

    #[test]
    fn test_usage_stats() {
        let mut manager = MigrationTokenManager::new(create_test_config());
        let source = Uuid::new_v4();

        // Generate a few tokens
        for i in 0..2 {
            let _token = manager.generate_token(
                Uuid::new_v4(),
                source,
                Uuid::new_v4(),
                TokenType::StateTransfer,
            ).unwrap();
        }

        let (count, first_used, last_used) = manager.get_usage_stats(source).unwrap();
        assert_eq!(count, 2);
        assert!(last_used >= first_used);
    }
}