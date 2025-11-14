use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerRequest {
    pub connection_id: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerResponse {
    pub success: bool,
    pub worker_addr: Option<String>,
    pub worker_label: Option<String>,
    pub connection_id: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DirectorError {
    NoWorkersAvailable,
    InvalidRequest(String),
}

#[rpcnet::service]
pub trait DirectorRegistry {
    async fn get_worker(&self, request: GetWorkerRequest) -> Result<GetWorkerResponse, DirectorError>;
}
