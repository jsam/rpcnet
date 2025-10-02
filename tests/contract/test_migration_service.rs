use rpcnet::migration::{
    MigrationService, MigrationServiceImpl, MigrationRequest, MigrationConfirmation, 
    ConnectionStateSnapshot, MigrationToken, ServerInstance, MigrationReason, ServerEndpoint
};
use rpcnet::RpcError;

type Result<T> = std::result::Result<T, RpcError>;
use uuid::Uuid;

#[tokio::test]
async fn test_initiate_migration_contract() {
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
    let target = ServerInstance::new("127.0.0.2".to_string(), 8083, 8082);
    
    let request = MigrationRequest::new(
        Uuid::new_v4(),
        source,
        target,
        MigrationReason::LoadBalancing,
        "test".to_string(),
    );

    let result = service.initiate_migration(request).await;
    assert!(result.is_ok(), "initiate_migration should return a valid token");
    
    let token = result.unwrap();
    assert!(token.is_active(), "Migration token should be active");
    assert!(!token.is_expired(), "Token should not be expired");
}

#[tokio::test]
async fn test_confirm_migration_contract() {
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
    let mut token = MigrationToken::for_migration(Uuid::new_v4(), issuer);
    token.activate();

    let result = service.confirm_migration(token).await;
    assert!(result.is_ok(), "confirm_migration should return confirmation");
    
    let confirmation = result.unwrap();
    assert!(confirmation.is_acceptance(), "Migration should be confirmed");
}

#[tokio::test]
async fn test_transfer_state_contract() {
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
    let mut token = MigrationToken::for_migration(Uuid::new_v4(), issuer);
    token.activate();

    let result = service.transfer_state(token).await;
    assert!(result.is_ok(), "transfer_state should return connection state");
    
    let state = result.unwrap();
    assert!(!state.connection_id.is_empty(), "Connection ID should not be empty");
}

#[tokio::test]
async fn test_complete_migration_contract() {
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    let confirmation = MigrationConfirmation::auto_accept(Uuid::new_v4());

    let result = service.complete_migration(confirmation).await;
    assert!(result.is_ok(), "complete_migration should succeed");
}

#[tokio::test]
async fn test_rollback_migration_contract() {
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
    let session_id = Uuid::new_v4();
    let mut token = MigrationToken::for_migration(session_id, issuer);
    token.activate();

    let _session_id = service.session_manager.create_session(Uuid::new_v4(), std::time::Duration::from_secs(30)).await.unwrap();

    let result = service.rollback_migration(token).await;
    assert!(result.is_ok(), "rollback_migration should succeed");
}