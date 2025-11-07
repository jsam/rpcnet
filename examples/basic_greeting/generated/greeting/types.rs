//! Type definitions for the service.
use serde::{Deserialize, Serialize};
/// Errors that can occur in greeting operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GreetingError {
    /// Empty name provided.
    EmptyName,
    /// Invalid input provided.
    InvalidInput(String),
}
/// Response from greeting operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetResponse {
    pub message: String,
}
/// Request for greeting operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetRequest {
    pub name: String,
}
