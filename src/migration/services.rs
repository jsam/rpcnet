use crate::migration::types::*;
use crate::RpcError;
use async_trait::async_trait;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, RpcError>;

#[async_trait]
pub trait MigrationService {
    async fn initiate_migration(&self, request: MigrationRequest) -> Result<MigrationToken>;
    async fn confirm_migration(&self, token: MigrationToken) -> Result<MigrationConfirmation>;
    async fn transfer_state(&self, token: MigrationToken) -> Result<ConnectionStateSnapshot>;
    async fn complete_migration(&self, confirmation: MigrationConfirmation) -> Result<()>;
    async fn rollback_migration(&self, token: MigrationToken) -> Result<()>;
}

#[async_trait]
pub trait HealthService {
    async fn health_check(&self, server_id: Uuid) -> Result<HealthStatus>;
    async fn get_migration_metrics(&self, server_id: Uuid) -> Result<MigrationMetrics>;
}

#[async_trait]
pub trait ServerConfigService {
    async fn configure_server_ports(&self, config: ServerConfig) -> Result<()>;
}

#[async_trait]
pub trait ConnectionService {
    async fn establish_connection(&self, request: ConnectionRequest) -> Result<ConnectionResponse>;
}