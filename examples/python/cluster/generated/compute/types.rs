//! Type definitions for the service.
use serde::{Deserialize, Serialize};
/// Errors that can occur during computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeError {
    WorkerBusy,
    InvalidInput(String),
    ProcessingFailed(String),
}
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
