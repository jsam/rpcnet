//! Type definitions for the service.
use serde::{Deserialize, Serialize};
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
/// Request to get an available worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerRequest {
    pub client_id: String,
}
