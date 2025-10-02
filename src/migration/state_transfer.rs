use crate::migration::models::ConnectionStateSnapshot;
use crate::migration::state::{
    encryption::{EncryptionService, EncryptionConfig},
    serialization::{SerializationService, SerializationConfig}
};

pub struct StateTransferService {
    _encryption_service: EncryptionService,
    _serialization_service: SerializationService,
}

impl StateTransferService {
    pub fn new() -> Self {
        Self {
            _encryption_service: EncryptionService::new(EncryptionConfig::default()),
            _serialization_service: SerializationService::new(SerializationConfig::default()),
        }
    }

    pub async fn prepare_state_for_transfer(
        &self,
        _snapshot: &ConnectionStateSnapshot,
    ) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }

    pub async fn receive_state(
        &self,
        _data: &[u8],
    ) -> Result<ConnectionStateSnapshot, String> {
        Ok(ConnectionStateSnapshot::new(uuid::Uuid::new_v4()))
    }
}

impl Default for StateTransferService {
    fn default() -> Self {
        Self::new()
    }
}