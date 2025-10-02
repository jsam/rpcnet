use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

// Encryption configuration and state
pub type EncryptionResult<T> = Result<T, EncryptionError>;

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    
    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),
    
    #[error("Key exchange failed: {0}")]
    KeyExchangeFailed(String),
    
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
    
    #[error("Invalid nonce format: {0}")]
    InvalidNonceFormat(String),
    
    #[error("Key expired")]
    KeyExpired,
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Random number generation failed: {0}")]
    RandomGenerationFailed(String),
}

// Encryption method specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionMethod {
    None,
    Aes256Gcm,
    ChaCha20Poly1305,
}

impl Default for EncryptionMethod {
    fn default() -> Self {
        EncryptionMethod::Aes256Gcm
    }
}

// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub method: EncryptionMethod,
    pub key_rotation_interval: Duration,
    pub max_key_age: Duration,
    pub require_authentication: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            method: EncryptionMethod::default(),
            key_rotation_interval: Duration::from_secs(3600), // 1 hour
            max_key_age: Duration::from_secs(7200), // 2 hours
            require_authentication: true,
        }
    }
}

// Encryption key with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub key_id: Uuid,
    pub key_data: Vec<u8>,
    pub method: EncryptionMethod,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub usage_count: u64,
    pub max_usage_count: Option<u64>,
}

impl EncryptionKey {
    pub fn new(method: EncryptionMethod, validity_duration: Duration) -> EncryptionResult<Self> {
        let key_data = Self::generate_key_data(method)?;
        let now = SystemTime::now();
        
        Ok(Self {
            key_id: Uuid::new_v4(),
            key_data,
            method,
            created_at: now,
            expires_at: now + validity_duration,
            usage_count: 0,
            max_usage_count: Some(1000), // Default usage limit
        })
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    pub fn is_usage_exceeded(&self) -> bool {
        if let Some(max_usage) = self.max_usage_count {
            self.usage_count >= max_usage
        } else {
            false
        }
    }

    pub fn can_use(&self) -> bool {
        !self.is_expired() && !self.is_usage_exceeded()
    }

    pub fn increment_usage(&mut self) {
        self.usage_count += 1;
    }

    pub fn remaining_validity(&self) -> Duration {
        self.expires_at.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO)
    }

    fn generate_key_data(method: EncryptionMethod) -> EncryptionResult<Vec<u8>> {
        match method {
            EncryptionMethod::None => Ok(Vec::new()),
            EncryptionMethod::Aes256Gcm => {
                let key = Aes256Gcm::generate_key(&mut OsRng);
                Ok(key.to_vec())
            },
            EncryptionMethod::ChaCha20Poly1305 => {
                // For now, use the same key size as AES-256
                let key = Aes256Gcm::generate_key(&mut OsRng);
                Ok(key.to_vec())
            },
        }
    }
}

// Encrypted data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub encryption_method: EncryptionMethod,
    pub key_id: Uuid,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag: Option<Vec<u8>>, // Authentication tag for AEAD
    pub encrypted_at: SystemTime,
    pub metadata: EncryptionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    pub original_size: usize,
    pub compressed_before_encryption: bool,
    pub checksum: Vec<u8>, // Checksum of original data
}

// State encryption service
pub struct StateEncryption {
    config: EncryptionConfig,
    active_keys: std::collections::HashMap<Uuid, EncryptionKey>,
    current_key_id: Option<Uuid>,
}

pub type EncryptionService = StateEncryption;

impl StateEncryption {
    pub fn new(config: EncryptionConfig) -> Self {
        Self {
            config,
            active_keys: std::collections::HashMap::new(),
            current_key_id: None,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(EncryptionConfig::default())
    }

    // Key management
    pub fn generate_new_key(&mut self) -> EncryptionResult<Uuid> {
        let key = EncryptionKey::new(self.config.method, self.config.max_key_age)?;
        let key_id = key.key_id;
        
        self.active_keys.insert(key_id, key);
        self.current_key_id = Some(key_id);
        
        Ok(key_id)
    }

    pub fn rotate_keys(&mut self) -> EncryptionResult<()> {
        // Generate new key
        self.generate_new_key()?;
        
        // Remove expired keys
        let now = SystemTime::now();
        self.active_keys.retain(|_, key| !key.is_expired());
        
        Ok(())
    }

    pub fn get_current_key(&mut self) -> EncryptionResult<&mut EncryptionKey> {
        // Check if we need to rotate keys
        if self.should_rotate_keys() {
            self.rotate_keys()?;
        }

        let key_id = self.current_key_id.ok_or_else(|| {
            EncryptionError::KeyDerivationFailed("No current key available".to_string())
        })?;

        self.active_keys.get_mut(&key_id).ok_or_else(|| {
            EncryptionError::KeyDerivationFailed("Current key not found".to_string())
        })
    }

    pub fn get_key(&self, key_id: &Uuid) -> EncryptionResult<&EncryptionKey> {
        self.active_keys.get(key_id).ok_or_else(|| {
            EncryptionError::KeyDerivationFailed(format!("Key {} not found", key_id))
        })
    }

    fn should_rotate_keys(&self) -> bool {
        if let Some(key_id) = self.current_key_id {
            if let Some(key) = self.active_keys.get(&key_id) {
                let age = SystemTime::now().duration_since(key.created_at).unwrap_or(Duration::ZERO);
                age >= self.config.key_rotation_interval || !key.can_use()
            } else {
                true // Key missing, need to rotate
            }
        } else {
            true // No current key, need to generate one
        }
    }

    // Encryption operations
    pub fn encrypt(&mut self, data: &[u8]) -> EncryptionResult<EncryptedData> {
        if self.config.method == EncryptionMethod::None {
            return Ok(self.create_unencrypted_container(data));
        }

        let metadata = self.create_metadata(data);
        let key = self.get_current_key()?;
        key.increment_usage();
        
        let key_method = key.method;
        let key_clone = EncryptionKey {
            key_id: key.key_id,
            key_data: key.key_data.clone(),
            method: key.method,
            created_at: key.created_at,
            expires_at: key.expires_at,
            usage_count: key.usage_count,
            max_usage_count: key.max_usage_count,
        };
        
        match key_method {
            EncryptionMethod::None => Ok(self.create_unencrypted_container(data)),
            EncryptionMethod::Aes256Gcm => self.encrypt_aes256_gcm(data, &key_clone, metadata),
            EncryptionMethod::ChaCha20Poly1305 => {
                // Fallback to AES for now
                self.encrypt_aes256_gcm(data, &key_clone, metadata)
            },
        }
    }

    pub fn decrypt(&self, encrypted_data: &EncryptedData) -> EncryptionResult<Vec<u8>> {
        if encrypted_data.encryption_method == EncryptionMethod::None {
            return Ok(encrypted_data.ciphertext.clone());
        }

        let key = self.get_key(&encrypted_data.key_id)?;
        
        if !key.can_use() {
            return Err(EncryptionError::KeyExpired);
        }

        match encrypted_data.encryption_method {
            EncryptionMethod::None => Ok(encrypted_data.ciphertext.clone()),
            EncryptionMethod::Aes256Gcm => self.decrypt_aes256_gcm(encrypted_data, key),
            EncryptionMethod::ChaCha20Poly1305 => {
                // Fallback to AES for now
                self.decrypt_aes256_gcm(encrypted_data, key)
            },
        }
    }

    // Convenience methods for common operations
    pub fn encrypt_state_snapshot(&mut self, snapshot_data: &[u8]) -> EncryptionResult<EncryptedData> {
        self.encrypt(snapshot_data)
    }

    pub fn decrypt_state_snapshot(&self, encrypted_data: &EncryptedData) -> EncryptionResult<Vec<u8>> {
        self.decrypt(encrypted_data)
    }

    pub fn encrypt_migration_token(&mut self, token_data: &[u8]) -> EncryptionResult<EncryptedData> {
        self.encrypt(token_data)
    }

    pub fn decrypt_migration_token(&self, encrypted_data: &EncryptedData) -> EncryptionResult<Vec<u8>> {
        self.decrypt(encrypted_data)
    }

    // Internal encryption implementations
    fn encrypt_aes256_gcm(
        &self, 
        data: &[u8], 
        key: &EncryptionKey, 
        metadata: EncryptionMetadata
    ) -> EncryptionResult<EncryptedData> {
        if key.key_data.len() != 32 {
            return Err(EncryptionError::InvalidKeyFormat("AES-256 requires 32-byte key".to_string()));
        }

        let key_array = Key::<Aes256Gcm>::from_slice(&key.key_data);
        let cipher = Aes256Gcm::new(key_array);
        
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        let ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        Ok(EncryptedData {
            encryption_method: EncryptionMethod::Aes256Gcm,
            key_id: key.key_id,
            nonce: nonce.to_vec(),
            ciphertext,
            tag: None, // Included in ciphertext for AEAD
            encrypted_at: SystemTime::now(),
            metadata,
        })
    }

    fn decrypt_aes256_gcm(&self, encrypted_data: &EncryptedData, key: &EncryptionKey) -> EncryptionResult<Vec<u8>> {
        if key.key_data.len() != 32 {
            return Err(EncryptionError::InvalidKeyFormat("AES-256 requires 32-byte key".to_string()));
        }

        if encrypted_data.nonce.len() != 12 {
            return Err(EncryptionError::InvalidNonceFormat("AES-256-GCM requires 12-byte nonce".to_string()));
        }

        let key_array = Key::<Aes256Gcm>::from_slice(&key.key_data);
        let cipher = Aes256Gcm::new(key_array);
        
        let nonce = Nonce::from_slice(&encrypted_data.nonce);
        
        let plaintext = cipher.decrypt(nonce, encrypted_data.ciphertext.as_ref())
            .map_err(|_| EncryptionError::AuthenticationFailed)?;

        // Verify checksum if required
        if self.config.require_authentication {
            let computed_checksum = self.compute_checksum(&plaintext);
            if computed_checksum != encrypted_data.metadata.checksum {
                return Err(EncryptionError::AuthenticationFailed);
            }
        }

        Ok(plaintext)
    }

    fn create_unencrypted_container(&self, data: &[u8]) -> EncryptedData {
        EncryptedData {
            encryption_method: EncryptionMethod::None,
            key_id: Uuid::nil(),
            nonce: Vec::new(),
            ciphertext: data.to_vec(),
            tag: None,
            encrypted_at: SystemTime::now(),
            metadata: self.create_metadata(data),
        }
    }

    fn create_metadata(&self, data: &[u8]) -> EncryptionMetadata {
        EncryptionMetadata {
            original_size: data.len(),
            compressed_before_encryption: false, // Could be detected from serialization layer
            checksum: self.compute_checksum(data),
        }
    }

    fn compute_checksum(&self, data: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    // Key management utilities
    pub fn cleanup_expired_keys(&mut self) {
        let now = SystemTime::now();
        self.active_keys.retain(|_, key| !key.is_expired());
        
        // If current key was removed, clear the reference
        if let Some(current_key_id) = self.current_key_id {
            if !self.active_keys.contains_key(&current_key_id) {
                self.current_key_id = None;
            }
        }
    }

    pub fn get_key_info(&self, key_id: &Uuid) -> Option<KeyInfo> {
        self.active_keys.get(key_id).map(|key| KeyInfo {
            key_id: key.key_id,
            method: key.method,
            created_at: key.created_at,
            expires_at: key.expires_at,
            usage_count: key.usage_count,
            max_usage_count: key.max_usage_count,
            is_current: self.current_key_id == Some(key.key_id),
        })
    }

    pub fn list_active_keys(&self) -> Vec<KeyInfo> {
        self.active_keys.values().map(|key| KeyInfo {
            key_id: key.key_id,
            method: key.method,
            created_at: key.created_at,
            expires_at: key.expires_at,
            usage_count: key.usage_count,
            max_usage_count: key.max_usage_count,
            is_current: self.current_key_id == Some(key.key_id),
        }).collect()
    }

    pub fn get_encryption_overhead(&self) -> usize {
        match self.config.method {
            EncryptionMethod::None => 0,
            EncryptionMethod::Aes256Gcm => 12 + 16, // Nonce + Tag
            EncryptionMethod::ChaCha20Poly1305 => 12 + 16, // Similar overhead
        }
    }
}

// Key information for monitoring and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: Uuid,
    pub method: EncryptionMethod,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub usage_count: u64,
    pub max_usage_count: Option<u64>,
    pub is_current: bool,
}

// Utility functions
pub mod utils {
    use super::*;

    pub fn quick_encrypt(data: &[u8]) -> EncryptionResult<EncryptedData> {
        let mut encryption = StateEncryption::with_default_config();
        encryption.encrypt(data)
    }

    pub fn quick_decrypt(encrypted_data: &EncryptedData, key: &EncryptionKey) -> EncryptionResult<Vec<u8>> {
        let mut encryption = StateEncryption::with_default_config();
        encryption.active_keys.insert(key.key_id, key.clone());
        encryption.decrypt(encrypted_data)
    }

    pub fn create_test_key() -> EncryptionResult<EncryptionKey> {
        EncryptionKey::new(EncryptionMethod::Aes256Gcm, Duration::from_secs(3600))
    }

    pub fn estimate_encrypted_size(original_size: usize, method: EncryptionMethod) -> usize {
        let overhead = match method {
            EncryptionMethod::None => 0,
            EncryptionMethod::Aes256Gcm => 12 + 16, // Nonce + Tag
            EncryptionMethod::ChaCha20Poly1305 => 12 + 16,
        };
        original_size + overhead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key = EncryptionKey::new(EncryptionMethod::Aes256Gcm, Duration::from_secs(3600)).unwrap();
        assert_eq!(key.key_data.len(), 32);
        assert_eq!(key.method, EncryptionMethod::Aes256Gcm);
        assert!(key.can_use());
    }

    #[test]
    fn test_key_expiration() {
        let key = EncryptionKey::new(EncryptionMethod::Aes256Gcm, Duration::from_millis(1)).unwrap();
        std::thread::sleep(Duration::from_millis(2));
        assert!(key.is_expired());
        assert!(!key.can_use());
    }

    #[test]
    fn test_encryption_roundtrip() {
        let mut encryption = StateEncryption::with_default_config();
        let data = b"Hello, World!";
        
        let encrypted = encryption.encrypt(data).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }

    #[test]
    fn test_no_encryption() {
        let config = EncryptionConfig {
            method: EncryptionMethod::None,
            ..Default::default()
        };
        let mut encryption = StateEncryption::new(config);
        let data = b"Hello, World!";
        
        let encrypted = encryption.encrypt(data).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
        assert_eq!(encrypted.encryption_method, EncryptionMethod::None);
    }

    #[test]
    fn test_key_rotation() {
        let mut encryption = StateEncryption::with_default_config();
        
        let key1_id = encryption.generate_new_key().unwrap();
        let key2_id = encryption.rotate_keys().unwrap();
        
        assert_ne!(key1_id, encryption.current_key_id.unwrap());
        assert!(encryption.active_keys.contains_key(&key1_id));
        assert!(encryption.active_keys.contains_key(&encryption.current_key_id.unwrap()));
    }

    #[test]
    fn test_authentication_failure() {
        let mut encryption = StateEncryption::with_default_config();
        let data = b"Hello, World!";
        
        let mut encrypted = encryption.encrypt(data).unwrap();
        // Tamper with ciphertext
        encrypted.ciphertext[0] ^= 1;
        
        let result = encryption.decrypt(&encrypted);
        assert!(matches!(result, Err(EncryptionError::AuthenticationFailed)));
    }

    #[test]
    fn test_key_usage_tracking() {
        let mut key = EncryptionKey::new(EncryptionMethod::Aes256Gcm, Duration::from_secs(3600)).unwrap();
        key.max_usage_count = Some(2);
        
        assert_eq!(key.usage_count, 0);
        assert!(key.can_use());
        
        key.increment_usage();
        assert_eq!(key.usage_count, 1);
        assert!(key.can_use());
        
        key.increment_usage();
        assert_eq!(key.usage_count, 2);
        assert!(!key.can_use());
    }

    #[test]
    fn test_expired_key_cleanup() {
        let mut encryption = StateEncryption::with_default_config();
        
        // Add expired key
        let expired_key = EncryptionKey::new(EncryptionMethod::Aes256Gcm, Duration::from_millis(1)).unwrap();
        let expired_key_id = expired_key.key_id;
        encryption.active_keys.insert(expired_key_id, expired_key);
        
        std::thread::sleep(Duration::from_millis(2));
        
        encryption.cleanup_expired_keys();
        assert!(!encryption.active_keys.contains_key(&expired_key_id));
    }

    #[test]
    fn test_metadata_creation() {
        let encryption = StateEncryption::with_default_config();
        let data = b"Test data";
        
        let metadata = encryption.create_metadata(data);
        assert_eq!(metadata.original_size, data.len());
        assert!(!metadata.checksum.is_empty());
    }

    #[test]
    fn test_encryption_overhead() {
        let encryption = StateEncryption::with_default_config();
        let overhead = encryption.get_encryption_overhead();
        assert_eq!(overhead, 28); // 12 (nonce) + 16 (tag)
    }

    #[test]
    fn test_utility_functions() {
        let data = b"Test data";
        let key = utils::create_test_key().unwrap();
        
        let estimated_size = utils::estimate_encrypted_size(data.len(), EncryptionMethod::Aes256Gcm);
        assert_eq!(estimated_size, data.len() + 28);
    }

    #[test]
    fn test_key_info() {
        let mut encryption = StateEncryption::with_default_config();
        let key_id = encryption.generate_new_key().unwrap();
        
        let info = encryption.get_key_info(&key_id).unwrap();
        assert_eq!(info.key_id, key_id);
        assert_eq!(info.method, EncryptionMethod::Aes256Gcm);
        assert!(info.is_current);
        
        let all_keys = encryption.list_active_keys();
        assert_eq!(all_keys.len(), 1);
        assert!(all_keys[0].is_current);
    }

    #[test]
    fn test_large_data_encryption() {
        let mut encryption = StateEncryption::with_default_config();
        let data = vec![0u8; 1_000_000]; // 1MB of zeros
        
        let encrypted = encryption.encrypt(&data).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        
        assert_eq!(data, decrypted);
        assert!(encrypted.ciphertext.len() > data.len()); // Should be larger due to encryption overhead
    }
}