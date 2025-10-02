use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub worker_id: Uuid,  // Stable worker ID
    pub label: String,
    pub user_addr: String,      // Port for RPC endpoints (generate, etc.)
    pub management_addr: String, // Port for health checks, registration
    pub capacity: usize,
    pub available: bool,  // Is worker ready to accept connections?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckRequest {
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub healthy: bool,
    pub worker_label: String,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterWorkerRequest {
    pub worker: WorkerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterWorkerResponse {
    pub success: bool,
    pub worker_id: Uuid,
}

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
pub struct InferenceRequest {
    pub connection_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceResponse {
    Connected { worker: String, connection_id: String },
    Token { text: String, sequence: u64 },
    Error { message: String },
    Done,
}
