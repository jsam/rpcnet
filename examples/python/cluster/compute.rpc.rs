use serde::{Deserialize, Serialize};

/// Request for compute task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub task_id: String,
    pub data: String,
}

/// Response from compute task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResponse {
    pub task_id: String,
    pub result: String,
    pub worker_id: String,
}

/// Errors that can occur during computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeError {
    WorkerBusy,
    InvalidInput(String),
    ProcessingFailed(String),
}

/// Compute service for worker nodes
#[rpcnet::service]
pub trait Compute {
    async fn process(&self, request: ComputeRequest) -> Result<ComputeResponse, ComputeError>;
}
