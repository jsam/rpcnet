//! Basic greeting service definition.

use serde::{Serialize, Deserialize};

/// Request for greeting operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetRequest {
    pub name: String,
}

/// Response from greeting operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetResponse {
    pub message: String,
}

/// Errors that can occur in greeting operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GreetingError {
    /// Empty name provided.
    EmptyName,
    /// Invalid input provided.
    InvalidInput(String),
}

/// Basic greeting service.
#[rpcnet::service]
pub trait Greeting {
    async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
}