use crate::migration::types::*;
use uuid::Uuid;

pub struct MigrationRequestHandler {
    server_id: Uuid,
}

impl MigrationRequestHandler {
    pub fn new(server_id: Uuid) -> Self {
        Self { server_id }
    }

    pub async fn handle_migration_request(
        &self,
        _message: MigrationMessage,
    ) -> Result<MigrationMessage, String> {
        Ok(MigrationMessage {
            message_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            timestamp: std::time::SystemTime::now(),
            source_server: self.server_id,
            target_server: Uuid::new_v4(),
            protocol_version: ProtocolVersion::default(),
            message_type: MigrationMessageType::InitiationResponse(MigrationInitiationResponse {
                request_id: Uuid::new_v4(),
                accepted: true,
                reason: None,
                estimated_duration: Some(std::time::Duration::from_secs(10)),
                supported_protocol_version: ProtocolVersion::default(),
                migration_token: None,
            }),
        })
    }
}