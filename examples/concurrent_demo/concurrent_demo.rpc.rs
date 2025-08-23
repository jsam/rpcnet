//! Concurrent operations demo service definition.

use serde::{Serialize, Deserialize};

/// Request for a CPU-intensive task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComputeRequest {
    pub task_id: String,
    pub iterations: u64,
}

/// Response from a computation task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComputeResponse {
    pub task_id: String,
    pub result: u64,
    pub duration_ms: u64,
}

/// Request for a simulated async task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsyncTaskRequest {
    pub task_id: String,
    pub delay_ms: u64,
}

/// Response from an async task.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsyncTaskResponse {
    pub task_id: String,
    pub completed_at: u64, // Unix timestamp
}

/// Request for counter increment (testing shared state).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IncrementRequest {
    pub amount: i64,
}

/// Response from counter increment.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IncrementResponse {
    pub new_value: i64,
}

/// Request to get current counter value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCounterRequest;

/// Response with current counter value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetCounterResponse {
    pub value: i64,
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

/// Service for testing concurrent operations and shared state.
#[rpcnet::service]
pub trait ConcurrentDemo {
    async fn compute(&self, request: ComputeRequest) -> Result<ComputeResponse, ConcurrentError>;
    async fn async_task(&self, request: AsyncTaskRequest) -> Result<AsyncTaskResponse, ConcurrentError>;
    async fn increment(&self, request: IncrementRequest) -> Result<IncrementResponse, ConcurrentError>;
    async fn get_counter(&self, request: GetCounterRequest) -> Result<GetCounterResponse, ConcurrentError>;
}