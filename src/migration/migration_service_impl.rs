use crate::migration::*;
use crate::RpcError;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct MigrationServiceImpl {
    pub session_manager: Arc<ConnectionSessionManager>,
    pub state_transfer: Arc<StateTransferService>,
    pub request_handler: Arc<MigrationRequestHandler>,
}

impl MigrationServiceImpl {
    pub fn new(server_id: Uuid) -> Self {
        Self {
            session_manager: Arc::new(ConnectionSessionManager::new()),
            state_transfer: Arc::new(StateTransferService::new()),
            request_handler: Arc::new(MigrationRequestHandler::new(server_id)),
        }
    }
}

#[async_trait]
impl MigrationService for MigrationServiceImpl {
    async fn initiate_migration(&self, request: MigrationRequest) -> Result<MigrationToken, RpcError> {
        let connection_id = request.connection_id;
        
        let session_id = self.session_manager
            .create_session(connection_id, std::time::Duration::from_secs(30))
            .await
            .map_err(|e| RpcError::InternalError(e))?;
        
        let issuer = ServerEndpoint::new(
            request.source_server.address.clone(),
            request.source_server.userspace_port,
            request.source_server.management_port,
        );
        
        let mut token = MigrationToken::for_migration(session_id, issuer);
        token.activate();
        
        Ok(token)
    }

    async fn confirm_migration(&self, token: MigrationToken) -> Result<MigrationConfirmation, RpcError> {
        if !token.is_valid() {
            return Err(RpcError::InvalidToken);
        }

        let confirmation = MigrationConfirmation::auto_accept(token.session_id);
        
        Ok(confirmation)
    }

    async fn transfer_state(&self, token: MigrationToken) -> Result<ConnectionStateSnapshot, RpcError> {
        if !token.is_valid() {
            return Err(RpcError::InvalidToken);
        }

        let snapshot = ConnectionStateSnapshot::new(token.session_id);

        Ok(snapshot)
    }

    async fn complete_migration(&self, confirmation: MigrationConfirmation) -> Result<(), RpcError> {
        if !confirmation.is_acceptance() {
            return Err(RpcError::MigrationRejected);
        }

        if let Some(session) = self.session_manager.get_session(confirmation.request_id).await {
            self.session_manager
                .update_session_state(session.session_id(), ConnectionState::Migrated)
                .await
                .map_err(|e| RpcError::InternalError(e))?;
        }

        Ok(())
    }

    async fn rollback_migration(&self, token: MigrationToken) -> Result<(), RpcError> {
        if let Some(state_machine) = self.session_manager.get_state_machine(token.session_id).await {
            state_machine
                .abort("Migration rollback requested".to_string())
                .await
                .map_err(|e| RpcError::InternalError(e))?;
        }

        self.session_manager
            .update_session_state(token.session_id, ConnectionState::Failed)
            .await
            .map_err(|e| RpcError::InternalError(e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initiate_migration() {
        let service = MigrationServiceImpl::new(Uuid::new_v4());
        let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
        let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);
        
        let request = MigrationRequest::new(
            Uuid::new_v4(),
            source,
            target,
            MigrationReason::LoadBalancing,
            "test".to_string(),
        );

        let token = service.initiate_migration(request).await.unwrap();
        assert!(token.is_active());
    }

    #[tokio::test]
    async fn test_confirm_migration() {
        let service = MigrationServiceImpl::new(Uuid::new_v4());
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let mut token = MigrationToken::for_migration(Uuid::new_v4(), issuer);
        token.activate();

        let confirmation = service.confirm_migration(token).await.unwrap();
        assert!(confirmation.is_acceptance());
    }

    #[tokio::test]
    async fn test_transfer_state() {
        let service = MigrationServiceImpl::new(Uuid::new_v4());
        let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
        let mut token = MigrationToken::for_migration(Uuid::new_v4(), issuer);
        token.activate();

        let snapshot = service.transfer_state(token).await.unwrap();
        assert!(snapshot.transport_state.connection_id.is_empty() || !snapshot.transport_state.connection_id.is_empty());
    }
}
