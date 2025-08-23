//! Type definitions for the service.
use serde::{Serialize, Deserialize};
/// Response from an async task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsyncTaskResponse {
    pub task_id: String,
    pub completed_at: u64,
}
/// Errors that can occur in concurrent operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConcurrentError {
    /// Task timeout.
    TaskTimeout,
    /// Invalid task parameters.
    InvalidParameters,
    /// System overload.
    SystemOverload,
}
/// Response with current counter value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCounterResponse {
    pub value: i64,
}
/// Request for a CPU-intensive task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComputeRequest {
    pub task_id: String,
    pub iterations: u64,
}
/// Response from counter increment.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IncrementResponse {
    pub new_value: i64,
}
/// Request for a simulated async task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsyncTaskRequest {
    pub task_id: String,
    pub delay_ms: u64,
}
/// Request for counter increment (testing shared state).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IncrementRequest {
    pub amount: i64,
}
/// Request to get current counter value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCounterRequest;
/// Response from a computation task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComputeResponse {
    pub task_id: String,
    pub result: u64,
    pub duration_ms: u64,
}
