use serde::{Deserialize, Serialize};

/// Request to get an available worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerRequest {
    pub client_id: String,
}

/// Response with worker information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerResponse {
    pub worker_addr: String,
    pub worker_id: String,
}

/// Errors from registry operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryError {
    NoWorkersAvailable,
    InvalidRequest(String),
}

/// Registry service for the director
#[rpcnet::service]
pub trait Registry {
    async fn get_worker(&self, request: GetWorkerRequest) -> Result<GetWorkerResponse, RegistryError>;
}
